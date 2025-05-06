use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct AppArgsData {
    /// This is data_1
    #[clap(short = 'n', long)]
    data_1: String,
    /// This is data_2
    #[clap(long)]
    data_2: Option<String>,
    /// This is data_3
    #[clap(long)]
    data_3: Option<String>,
    /// Subcommand womp womp nigger
    #[clap(subcommand)]
    pub data_4: EntityType,
}

#[derive(Debug, Subcommand)]
pub enum EntityType {
    /// Fà della roba da negro
    User(UserCommand),

    /// Puttana dio
    Woke,
}

#[derive(Debug, Args)]
pub struct UserCommand {
    #[clap(subcommand)]
    pub command: UserSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum UserSubCommand {
    Create(CreateUser),
}

#[derive(Debug, Args)]
pub struct CreateUser {
    /// Porco dio
    pub name: String,
    /// Porca madonna
    pub email: String,
}

fn main() {
    let parsed_data = AppArgsData::parse();
    dbg!(parsed_data);
}
