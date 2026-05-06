mod utils;
mod structs;

mod commands;

// use std::env;
use clap::{Parser, Subcommand};
use commands::{
    command_init,
    command_cat_file,
    command_hash_object,
    command_ls_tree,
    command_write_tree,
    command_commit_tree,
    command_clone,
};

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initializes a new git repository
    Init,
    /// Cat a file from the object database
    CatFile {
        /// Pretty print the file contents
        #[arg(short)]
        p: bool,
        /// The file to cat
        file: String
    },
    /// Hash a file and store it in the object database
    HashObject {
        /// Write the hash to the object database
        #[arg(short)]
        w: bool,
        /// The file to hash
        file: String
    },
    /// List the contents of a tree object
    LsTree {
        /// Print only names
        #[arg(long)]
        name_only: bool,
        /// The tree object to list
        object_id: String,
    },
    /// Write a tree object
    WriteTree,
    /// Commit tree
    CommitTree {
        /// The parent commit
        #[arg(short)]
        p: String,
        /// The commit message
        #[arg(short)]
        m: String,
        /// The tree object to commit
        tree: String,
    },
    /// Clone a remote repository
    Clone {
        /// The remote repository URL
        url: String,
        /// The local directory to clone into
        dir: String,
    },
}

/// Git CLI
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Git command to execute
    #[command(subcommand)]
    command: Commands,
}

fn main() {
    let cli = Args::parse();
    match &cli.command {
        Commands::Init => {
            if let Err(e) = command_init(".") {
                eprintln!("Error: {}", e);
            }
        },
        // TODO: p & w are not implemented yet
        Commands::CatFile { p, file } => {
            let content = command_cat_file(file, ".");
            print!("{}", content);
        },
        Commands::HashObject { w, file } => {
            let hex_hash = command_hash_object(file, ".");
            println!("{}", hex_hash)
        },
        Commands::LsTree { name_only, object_id } => {
            let _ = command_ls_tree(object_id, name_only, ".");
        },
        Commands::WriteTree => {
            let root = std::path::Path::new(".");
            let hex_hash = command_write_tree(root, ".");
            println!("{}", hex_hash);
        },
        Commands::CommitTree { p, m, tree } => {
            command_commit_tree(&tree, &p, &m);
        },
        Commands::Clone { url, dir } => {
            command_clone(url, dir);
        },
    }
}
