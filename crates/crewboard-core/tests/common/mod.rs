//! Shared fixtures for the board tests: a store in a temporary data folder and temporary git repositories.
#![allow(dead_code)]

use crewboard_core::board::Board;
use crewboard_core::board::projects::AddProject;
use crewboard_core::board::tickets::{NewTicketInput, Ticket};
use crewboard_core::data_dir::DataDir;
use crewboard_core::domain::{Actor, Kind};
use crewboard_core::git::testing::TempRepos;
use crewboard_core::store::Store;

pub struct Fixture {
    pub home: tempfile::TempDir,
    pub repos: TempRepos,
    pub board: Board,
}

impl Fixture {
    pub async fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let store = Store::open_in(&DataDir::new(home.path().join("home")))
            .await
            .unwrap();
        Self {
            home,
            repos: TempRepos::new(),
            board: Board::with_path_git(store),
        }
    }

    /// A fixture with the `shop-web` project (key SHOP, base `main`).
    pub async fn shop() -> Self {
        let f = Self::new().await;
        f.add_project("shop-web").await;
        f
    }

    pub async fn add_project(&self, name: &str) {
        let repo = self.repos.repo(name, 1);
        self.board
            .add_project(&AddProject {
                path: repo.display().to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    pub async fn create(&self, title: &str) -> Ticket {
        self.create_with(NewTicketInput {
            title: title.into(),
            ..shop_input()
        })
        .await
    }

    pub async fn epic(&self, title: &str) -> Ticket {
        self.create_with(NewTicketInput {
            title: title.into(),
            kind: Kind::Epic,
            ..shop_input()
        })
        .await
    }

    pub async fn child(&self, epic: &Ticket, title: &str) -> Ticket {
        self.create_with(NewTicketInput {
            title: title.into(),
            epic: Some(epic.key.to_string()),
            ..shop_input()
        })
        .await
    }

    pub async fn create_with(&self, input: NewTicketInput) -> Ticket {
        self.board.create_ticket(&input, &me()).await.unwrap()
    }

    /// Creates tickets until the next one would get `number`.
    pub async fn skip_to(&self, number: u32) {
        loop {
            let t = self.create("filler").await;
            if t.key.number + 1 >= number {
                break;
            }
        }
    }

    pub async fn ticket(&self, key: &str) -> Ticket {
        self.board.ticket(key).await.unwrap()
    }
}

pub fn shop_input() -> NewTicketInput {
    NewTicketInput {
        project: "SHOP".into(),
        ..Default::default()
    }
}

pub fn me() -> Actor {
    Actor::User("me".into())
}

pub fn agent(key: &str) -> Actor {
    Actor::Agent(key.parse().unwrap())
}

impl Fixture {
    /// The data folder the fixture's store lives in.
    pub fn data(&self) -> crewboard_core::data_dir::DataDir {
        DataDir::new(self.home.path().join("home"))
    }

    /// The `shop-web` repository.
    pub fn repo(&self) -> std::path::PathBuf {
        self.repos.root().join("shop-web")
    }

    /// Gives a ticket its branch and worktree as the start sequence does; returns the worktree.
    pub async fn start_worktree(&self, ticket: &Ticket) -> std::path::PathBuf {
        crewboard_core::start::prepare_worktree(&self.board, &self.data(), &ticket.key.to_string())
            .await
            .unwrap()
            .path
    }

    /// Puts a ticket straight into `status`, as if the board had got it there.
    pub async fn force_status(&self, ticket: &Ticket, status: crewboard_core::domain::Status) {
        sqlx::query("UPDATE tickets SET board_column = ?, state = ? WHERE id = ?")
            .bind(status.column().as_str())
            .bind(status.as_str())
            .bind(&ticket.id)
            .execute(self.board.store().pool())
            .await
            .unwrap();
    }
}
