use std::env;
use std::fs;
use std::io::prelude::*;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Sha1, Digest};

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
    } else {
        println!("unknown command: {}", args[1])
    }
}
