#[macro_use]
extern crate prettytable;
mod cli;

use cli::Cli;

use colored::*;
use dotenvy::dotenv;
use futures::{stream::FuturesUnordered, StreamExt};
use octocrab::{
    models::{pulls::Comment, teams::Team, User},
    Octocrab, Page, Result,
};
use prettytable::Table;
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::Arc,
};
use tokio::task::{JoinHandle, JoinSet};

type GhApiPullRequest = octocrab::models::pulls::PullRequest;

struct PullRequest<'a>(&'a GhApiPullRequest);

impl Hash for PullRequest<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.number.hash(state);
    }
}

impl PartialEq for PullRequest<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.number == other.0.number
    }
}

impl Eq for PullRequest<'_> {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_thread_ids(true)
        .with_ansi(true)
        .init();

    let token = std::env::var("GH_TOKEN").expect("GH_TOKEN must be set");

    let octocrab = Arc::new(
        octocrab::Octocrab::builder()
            .personal_token(token)
            .build()
            .unwrap(),
    );

    let args = Cli::get_args();

    let repo = args.repo;
    let mut repo_parts = repo.split('/');

    let (owner, repo) = match (repo_parts.next(), repo_parts.next()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        (Some(repo), None) => (
            {
                let me = octocrab.current().user().await?;

                me.login.to_owned()
            },
            repo.to_string(),
        ),
        _ => {
            eprintln!("Invalid repository name");
            std::process::exit(1);
        }
    };

    let teams = octocrab.get(format!("/user/teams"), None::<&()>).await?;

    let teams: Vec<Team> = serde_json::from_value(teams).unwrap();

    let my_teams: Vec<&Team> = teams
        .iter()
        .filter(|&team| {
            team.organization
                .as_ref()
                .map_or(false, |org| org.login == owner)
        })
        .collect();

    let me = octocrab.current().user().await?;

    let pulls = octocrab.pulls(&owner, &repo);

    let prs = pulls
        .list()
        .per_page(args.last)
        .state(args.state.into())
        .send()
        .await?;

    let mut prs_concerning_me: HashSet<PullRequest> = HashSet::new();

    tracing::info!("Processing PRs...");
    for pr in &prs {
        let show = process_pr(&pr, my_teams.clone(), &me);

        if show {
            prs_concerning_me.insert(PullRequest(pr));
        }
    }

    if args.comments {
        tracing::info!("Getting comments for PRs...");

        let prs_mentioning_me =
            comments_mention_me(octocrab.clone(), &prs, &owner, &repo, &me.login).await?;

        prs_mentioning_me.into_iter().for_each(|pr| {
            prs_concerning_me.insert(PullRequest(pr));
        });
    }

    tracing::info!("Getting additions and deletions for PRs...");
    let pr_additions_and_deletions =
        get_additions_deletions(octocrab.clone(), &owner, &repo, &prs_concerning_me).await?;

    render(prs_concerning_me, pr_additions_and_deletions)
}

fn process_pr(pr: &GhApiPullRequest, my_teams: Vec<&Team>, me: &User) -> bool {
    let requested_review = pr
        .requested_reviewers
        .iter()
        .flatten()
        .any(|reviewer| reviewer.login == me.login);

    let mentions_me = pr
        .body
        .as_ref()
        .unwrap_or(&"".to_string())
        .contains(&format!("@{}", me.login));

    let assigned_to_my_team = pr.requested_teams.iter().flatten().any(|team| {
        my_teams
            .iter()
            .any(|my_team| team.id.map_or(false, |id| id == my_team.id))
    });

    let assigned_to_me = pr
        .assignees
        .iter()
        .flatten()
        .any(|assignee| assignee.login == me.login);

    requested_review || mentions_me || assigned_to_my_team || assigned_to_me
}

async fn comments_mention_me<'a>(
    octocrab: Arc<Octocrab>,
    prs: &'a Page<GhApiPullRequest>,
    owner: &'a String,
    repo: &'a String,
    login: &'a String,
) -> Result<Vec<&'a GhApiPullRequest>> {
    let mut result: Vec<&GhApiPullRequest> = Vec::new();

    let mut sub_tasks: JoinSet<Result<(u64, Page<Comment>)>> = JoinSet::new();

    // Spawn a task for each PR to get the comments
    prs.items
        .iter()
        .map(|pr| {
            let pr_number = pr.number;
            let octocrab = octocrab.clone();
            let owner = owner.clone();
            let repo = repo.clone();

            sub_tasks.spawn(async move {
                let comments = octocrab
                    .pulls(owner, repo)
                    .list_comments(Some(pr_number))
                    .send()
                    .await?;

                Ok((pr_number, comments))
            })
        })
        .for_each(drop);

    // Process each task's result as it completes
    while let Some(res) = sub_tasks.join_next().await {
        if let Ok(r) = res {
            let (pr_number, comments) = r?;
            if comments.items.iter().any(|comment| {
                comment.body.contains(&format!("@{}", login))
                    || if let Some(user) = &comment.user {
                        user.login.eq(login)
                    } else {
                        false
                    }
            }) {
                let pr = prs.into_iter().find(|&p| p.number == pr_number).unwrap();
                result.push(&pr);
            }
        }
    }

    Ok(result)
}

async fn get_additions_deletions(
    octocrab: Arc<Octocrab>,
    owner: &String,
    repo: &String,
    prs: &HashSet<PullRequest<'_>>,
) -> Result<HashMap<u64, (u64, u64)>> {
    let mut results: HashMap<u64, (u64, u64)> = HashMap::new();

    let mut handles: FuturesUnordered<JoinHandle<Result<(u64, (u64, u64))>>> =
        FuturesUnordered::new();

    let pr_numbers: Vec<u64> = prs.iter().map(|pr| pr.0.number).collect();

    for pr_number in pr_numbers {
        let octocrab = octocrab.clone();
        let owner = owner.clone();
        let repo = repo.clone();
        let handle = tokio::spawn(async move {
            let files = octocrab.pulls(owner, repo).list_files(pr_number).await?;

            let (additions, deletions) = files.into_iter().fold((0, 0), |(a, d), file| {
                (a + file.additions, d + file.deletions)
            });

            Ok((pr_number, (additions, deletions)))
        });

        handles.push(handle);
    }

    while let Some(res) = handles.next().await {
        let (pr, (a, d)) = res.unwrap()?;

        results.insert(pr, (a, d));
    }

    Ok(results)
}

fn make_table_row(
    pr: &octocrab::models::pulls::PullRequest,
    (additions, deletions): (u64, u64),
) -> prettytable::Row {
    row![
        pr.number.to_string().bold(),
        pr.updated_at.map_or("Unknown".to_string(), |date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M %Z")
                .to_string()
                .yellow()
                .to_string()
        }),
        pr.title.as_ref().unwrap_or(&"No title".to_string()).bold(),
        pr.html_url
            .as_ref()
            .map_or("Unknown".to_string(), |url| url.to_string())
            .underline(),
        pr.user
            .as_ref()
            .map_or("Unknown".to_string(), |u| format!("@{}", u.login))
            .green(),
        format!(
            "+{}/-{}",
            additions.to_string().green(),
            deletions.to_string().red()
        )
    ]
}
fn render(
    prs: HashSet<PullRequest>,
    pr_additions_and_deletions: HashMap<u64, (u64, u64)>,
) -> Result<()> {
    let mut table1 = Table::new();
    table1.set_format(*prettytable::format::consts::FORMAT_CLEAN);

    let mut ordered_prs: Vec<&PullRequest> = prs.iter().collect();
    ordered_prs.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));

    table1.set_titles(row![
        "PR #",
        "Last Updated",
        "Title",
        "URL",
        "Author",
        "+/-"
    ]);

    for pr in ordered_prs {
        let pr = pr.0;

        let (additions, deletions) = pr_additions_and_deletions
            .get(&pr.number)
            .map_or((0, 0), |(a, d)| (*a, *d));

        table1.add_row(make_table_row(pr, (additions, deletions)));
    }

    table1.printstd();

    Ok(())
}
