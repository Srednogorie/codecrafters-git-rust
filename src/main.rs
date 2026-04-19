use std::env;
use std::fs;
use std::io::prelude::*;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Digest, Sha1};


fn get_file_sha(path: &std::path::Path) -> Vec<u8> {
    let contents = fs::read(path).unwrap();
    let header = format!("blob {}", contents.len());
    let data = [header.as_bytes(), &[0], &contents].concat();
    Sha1::digest(&data).to_vec()
}

fn write_tree(path: &std::path::Path) -> Vec<u8> {
    let mut entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().into_string().unwrap();

        if path.is_file() {
            let hash = get_file_sha(&path); // you already have this
            let header = format!("100644 {name}\0").into_bytes();
            entries.push((header, hash));
        } else if path.is_dir() {
            if name == ".git" {
                continue;
            }
            let hash = write_tree(&path); // recursion
            let header = format!("40000 {name}\0").into_bytes();
            entries.push((header, hash));
        }
    }
    
    // TODO: this cannot be! We need to refactor all this and find better and cleaner way to sort.
    // Git requires entries sorted by name, with directories compared as "name/"
    entries.sort_by(|a, b| {
        // Extract name from header: "<mode> <name>\0" → name is between space and null
        let name_of = |header: &Vec<u8>| -> Vec<u8> {
            let space_pos = header.iter().position(|&c| c == b' ').unwrap();
            let null_pos = header.iter().position(|&c| c == 0).unwrap();
            let is_dir = &header[..space_pos] == b"40000";
            let mut name = header[space_pos + 1..null_pos].to_vec();
            if is_dir {
                name.push(b'/');
            }
            name
        };
        name_of(&a.0).cmp(&name_of(&b.0))
    });

    // Build tree content
    let mut content = Vec::new();
    for (header, hash) in entries {
        content.extend_from_slice(&header);
        content.extend_from_slice(&hash);
    }

    // Hash the tree object
    let header = format!("tree {}\0", content.len());
    let mut data = header.into_bytes();
    data.extend_from_slice(&content);

    let hash = Sha1::digest(&data);
    let hex_hash = format!("{:x}", hash);
    let dir = format!(".git/objects/{}", &hex_hash[..2]);
    fs::create_dir_all(&dir).unwrap();
    let file_path = format!("{}/{}", dir, &hex_hash[2..]);
    let mut f = fs::File::create(file_path).unwrap();
    let mut compressed = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    compressed.write_all(&data).unwrap();
    let compressed_bytes = compressed.finish().unwrap();
    f.write_all(&compressed_bytes).unwrap();

    hash.to_vec()
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    eprintln!("Logs from your program will appear here!");

    // TODO: Uncomment the code below to pass the first stage
    let args: Vec<String> = env::args().collect();
    if args[1] == "init" {
        fs::create_dir(".git").unwrap();
        fs::create_dir(".git/objects").unwrap();
        fs::create_dir(".git/refs").unwrap();
        fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();
        println!("Initialized git directory")
    } else if args[1] == "cat-file" && args[2] == "-p" {
        let object_id = &args[3];
        let path = format!(".git/objects/{}/{}", &object_id[..2], &object_id[2..]);
        let mut d = ZlibDecoder::new(fs::File::open(path).unwrap());
        let mut bytes = Vec::new();
        d.read_to_end(&mut bytes).unwrap();
        
        // Find null byte separator - \x00 - blob 12\x00hello world
        // blob 12 → object header
        // \0 → null byte (not visible when printed)
        // hello world → actual file contents
        if let Some(null_pos) = bytes.iter().position(|&b| b == 0) {
            let content = &bytes[null_pos + 1..];
            print!("{}", String::from_utf8_lossy(content));
        }
    } else if args[1] == "hash-object" && args[2] == "-w" {
        let path = &args[3];
        let contents = fs::read(path).unwrap();
        let header = format!("blob {}", contents.len());
        let data = [header.as_bytes(), &[0], &contents].concat();
        let hash = Sha1::digest(&data);
        let hex_hash = format!("{:x}", hash);
        println!("{}", hex_hash);
        let dir = format!(".git/objects/{}", &hex_hash[..2]);
        fs::create_dir_all(&dir).unwrap();
        let file = format!("{}/{}", dir, &hex_hash[2..]);
        let mut f = fs::File::create(file).unwrap();
        let mut compressed = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        compressed.write_all(&data).unwrap();
        let compressed_bytes = compressed.finish().unwrap();
        f.write_all(&compressed_bytes).unwrap();
    } else if args[1] == "ls-tree" {
        let object_id = if args.len() == 4 { &args[3] } else { &args[2] };
        let path = format!(".git/objects/{}/{}", &object_id[..2], &object_id[2..]);
        let mut d = ZlibDecoder::new(fs::File::open(path).unwrap());
        let mut bytes = Vec::new();
        d.read_to_end(&mut bytes).unwrap();
        
        let mut line = Vec::new();
        let mut header_vec = Vec::new();
        let mut header = false;
        let mut current_bytes = Vec::new();
        let mut reading_bytes = false;
        let mut record = String::new();
        let mut records = Vec::new();
        for ch in bytes {
            
            if !header && ch != b'\0' {
                header_vec.push(ch);
                continue;
            }
            if !header && ch == b'\0' {
                header = true;
                continue;
            }
            // <mode> <filename>\0<20 bytes>
            if ch == b' ' && !reading_bytes {
                let mode = &line;
                record.push_str(&format!("{:0>6} ", String::from_utf8_lossy(&mode)));
                line.clear();
                continue;
            } else if ch == b'\0' && !reading_bytes {
                let filename = &line;
                record.push_str(&format!("{} ", String::from_utf8_lossy(&filename)));
                line.clear();
                reading_bytes = true;
                continue;
            } else {
                if reading_bytes && current_bytes.len() != 20 {
                    current_bytes.push(ch);
                } else {
                    if current_bytes.len() == 20 {
                        let hash = Sha1::digest(&current_bytes);
                        let hex_hash = format!("{:x}", hash);
                        record.push_str(&hex_hash);
                        current_bytes.clear();
                        reading_bytes = false;
                        
                        records.push(record.clone());
                        record.clear();
                    }
                    line.push(ch);
                }
            }
        }
        // Get the last record
        let hash = Sha1::digest(&current_bytes);
        let hex_hash = format!("{:x}", hash);
        record.push_str(&hex_hash);
        records.push(record.clone());
        
        // records.sort();
        if args.len() == 4 {
            if args[2] == "--name-only" {
                for record in records {
                    let spitted: Vec<&str> = record.split_whitespace().collect();
                    let filename = spitted[1];
                    println!("{}", filename);
                }
            }
        } else {
            for record in records {
                println!("{}", record);
            }
        }
    } else if args[1] == "write-tree" {
        let root = std::path::Path::new(".");
        let hash_bytes = write_tree(root);
        let hex_hash: String = hash_bytes.iter().map(|b| format!("{:02x}", b)).collect();
        println!("{}", hex_hash);
    } else if args[1] == "commit-tree" {
        // cargo run -- commit-tree 72bfbdef47b78ffe53ed08262696acf5c53eabc9 -p 554eef8599c84133a41e56f8317ad8807a9ec293 -m "Second commit"
        
        /*
        commit 177\0tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904
        parent 3b18e512dba79e4c8300dd08aeb37f8e728b8dad
        author John Doe <john@example.com> 1234567890 +0000
        committer John Doe <john@example.com> 1234567890 +0000
        
        Initial commit
        */
        
        let tree_sha = args[2].clone();
        let parent_sha = args[4].clone();
        let message = args[6].clone();
        
        let tree_sha_path = format!(".git/objects/{}/{}", &tree_sha[..2], &tree_sha[2..]);
        let mut tree_sha_d = ZlibDecoder::new(fs::File::open(tree_sha_path).unwrap());
        let mut tree_sha_bytes = Vec::new();
        tree_sha_d.read_to_end(&mut tree_sha_bytes).unwrap();
        let tree_sha_line_size = ["tree ".as_bytes(), &tree_sha_bytes, "\n".as_bytes()].concat();
        
        let parent_sha_path = format!(".git/objects/{}/{}", &parent_sha[..2], &parent_sha[2..]);
        let mut parent_sha_d = ZlibDecoder::new(fs::File::open(parent_sha_path).unwrap());
        let mut parent_sha_bytes = Vec::new();
        parent_sha_d.read_to_end(&mut parent_sha_bytes).unwrap();
        let parent_sha_line_size = ["parent ".as_bytes(), &parent_sha_bytes, "\n".as_bytes()].concat();
        
        let author_line_size = ["author John Doe <john@example.com> 1234567890 +0000\n".as_bytes()].concat();
        let committer_line_size = ["committer John Doe <john@example.com> 1234567890 +0000\n\n".as_bytes()].concat();
        let message_line_size = message.as_bytes();
        
        let total_line_size = 
            tree_sha_line_size.len() +
            parent_sha_line_size.len() +
            author_line_size.len() +
            committer_line_size.len() +
            message_line_size.len();
        
        let commit = format!("commit {}\0tree {}\nparent {}\nauthor John Doe <john@example.com> 1234567890 +0000\ncommitter John Doe <john@example.com> 1234567890 +0000\n\n{}\n", total_line_size, tree_sha, parent_sha, message);
        
        let data = commit.as_bytes();
        
        let hash = Sha1::digest(&data);
        let hex_hash = format!("{:x}", hash);
        println!("{}", hex_hash);
        let dir = format!(".git/objects/{}", &hex_hash[..2]);
        fs::create_dir_all(&dir).unwrap();
        let file = format!("{}/{}", dir, &hex_hash[2..]);
        let mut f = fs::File::create(file).unwrap();
        let mut compressed = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        compressed.write_all(&data).unwrap();
        let compressed_bytes = compressed.finish().unwrap();
        f.write_all(&compressed_bytes).unwrap();
    } else {
        println!("unknown command: {}", args[1])
    }
}
