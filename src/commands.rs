use std::fs;
use flate2::read::ZlibDecoder;
use std::io::{Read, Write};
use std::fs::File;
use sha1::{Digest, Sha1};
use crate::structs::TreeEntry;
use crate::utils::{
    apply_delta, get_file_blob, parse_tree_entries, read_git_object, read_pkt_lines, write_dir_to_disk, write_git_object
};

pub fn command_init(base_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir(format!("{}/.git", base_dir))?;
    fs::create_dir(format!("{}/.git/objects", base_dir))?;
    fs::create_dir(format!("{}/.git/refs", base_dir))?;
    fs::write(format!("{}/.git/HEAD", base_dir), "ref: refs/heads/main\n")?;
    println!("Initialized git directory");
    Ok(())
}

pub fn command_cat_file(object_id: &str, base_dir: &str) -> String {
    let bytes = read_git_object(base_dir, object_id);
    let null_pos = bytes.iter().position(|&b| b == 0)
        .expect("malformed git object: no null byte");
    String::from_utf8_lossy(&bytes[null_pos + 1..]).into_owned()
}

pub fn command_hash_object(path: &str, base_dir: &str) -> String {
    let file = File::open(path).unwrap();
    let data = get_file_blob(file);
    let hex_hash = write_git_object(base_dir, &data);
    hex_hash
}

pub fn command_ls_tree(object_id: &str, name_only: &bool, base_dir: &str) -> Vec<String> {
    let bytes = read_git_object(base_dir, object_id);
    
    let records = parse_tree_entries(&bytes);
    let mut to_print = Vec::new();

    if *name_only {
        for record in &records {
            let filename = record.name.clone();
            to_print.push(filename);
        }
    } else {
        for record in &records {
            to_print.push(format!("{} - {} - {}", record.mode, record.name, record.hash));
        }
    }
    for line in &to_print {
        println!("{}", line);
    }
    to_print
}

pub fn command_write_tree(path: &std::path::Path, base_dir: &str) -> String {
    let mut content = Vec::new();

    let mut sorted_entries: Vec<_> = fs::read_dir(path).unwrap().map(|e| e.unwrap()).collect();
    sorted_entries.sort_by_key(|a| {
        let mut name = a.file_name().into_string().unwrap();
        if a.path().is_dir() {
            name.push('/');
        }
        name
    });

    for entry in sorted_entries {
        let path = entry.path();
        let name = entry.file_name().into_string().unwrap();

        if path.is_file() {
            let file = File::open(&path).unwrap();
            let hash = Sha1::digest(&get_file_blob(file));
            let header = format!("100644 {name}\0").into_bytes();
            content.extend_from_slice(&header);
            content.extend_from_slice(&hash.to_vec());
        } else if path.is_dir() {
            if name == ".git" {
                continue;
            }
            let hash = command_write_tree(&path, base_dir); // recursion
            let header = format!("40000 {name}\0").into_bytes();
            content.extend_from_slice(&header);
            content.extend_from_slice(&hex::decode(&hash).unwrap());
        }
    }

    // Hash the tree object
    let header = format!("tree {}\0", content.len());
    let mut data = header.into_bytes();
    data.extend_from_slice(&content);

    let hex_hash = write_git_object(base_dir, &data);
    hex_hash
}

pub fn command_commit_tree(tree_sha: &str, parent_sha: &str, message: &str) {
    let content = format!(
        concat!(
            "tree {}\n",
            "parent {}\n",
            "author John Doe <john@example.com> 1234567890 +0000\n",
            "committer John Doe <john@example.com> 1234567890 +0000\n\n",
            "{}\n"
        ),
        tree_sha,
        parent_sha,
        message
    );
    
    let data = content.into_bytes();
    let mut commit_msg = format!("commit {}\0", data.len()).into_bytes();
    commit_msg.extend_from_slice(&data);
    
    let hex_hash = write_git_object(".", &commit_msg);
    println!("{}", hex_hash);
}

pub fn command_clone(repo_url: &str, clone_dir: &str) {
    // We first need to fetch the discovery reference
    let response = reqwest::blocking::get(format!("{}/info/refs?service=git-upload-pack", repo_url)).unwrap();
    let response_text = response.bytes().unwrap();
    let lines = read_pkt_lines(&response_text);
    // Having the lines of the discovery response we need to build the upload pack request message
    let parts: Vec<&[u8]> = lines[1].split(|b| *b == 0).collect();
    let hash = parts[0].split(|b| b.is_ascii_whitespace()).next().unwrap();
    // 
    let message = format!(
        "want {} multi_ack_detailed side-band-64k thin-pack ofs-delta\n",
        std::str::from_utf8(hash).unwrap()
    );
    let pkt_line = format!("{:04x}{}", message.len() + 4, message).into_bytes();
    
    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line);
    body.extend(b"0000");
    body.extend(b"0009done\n");

    let upload_pack_url = format!("{}/git-upload-pack", repo_url);
    let upload_pack_response = reqwest::blocking::Client::new()
        .post(&upload_pack_url)
        .body(body)
        .header("Content-Type", "application/x-git-upload-pack-request")
        .send()
        .unwrap();

    let upload_pack_lines = read_pkt_lines(&upload_pack_response.bytes().unwrap());
    let side_band_lines: Vec<Vec<u8>> = upload_pack_lines.into_iter().filter(|l| l[0] == 1).collect();
    let mut side_band_merged = Vec::new();
    for line in side_band_lines {
        side_band_merged.extend_from_slice(&line[1..]);
    }

    let num_objects = u32::from_be_bytes(side_band_merged[8..12].try_into().unwrap());
    
    let mut current_offset = 12;
    let mut pack_objects: std::collections::HashMap<usize, (String, Vec<u8>)> = std::collections::HashMap::new();

    for _ in 0..num_objects {
        let current_object_start = current_offset;
        let mut first_byte = &side_band_merged[current_offset];
        let type_byte = first_byte >> 4 & 0b111;
        let object_type = match type_byte {
            1 => "commit",
            2 => "tree",
            3 => "blob",
            4 => "tag",
            6 => "ofs_delta",
            7 => "ref_delta",
            _ => "unknown",
        };

        current_offset = current_offset + 1;
        while first_byte & 0b10000000 != 0 {
            first_byte = &side_band_merged[current_offset];
            current_offset += 1;
        }
        if object_type == "ofs_delta" {
            let mut ofs_byte = side_band_merged[current_offset];
            current_offset += 1;
            let mut delta_offset = (ofs_byte & 0x7f) as u32;
            while ofs_byte & 0x80 != 0 {
                ofs_byte = side_band_merged[current_offset];
                current_offset += 1;
                delta_offset = ((delta_offset + 1) << 7) | (ofs_byte & 0x7f) as u32;
            }
            let base_object_start = current_object_start - delta_offset as usize;
            
            let mut decoder = ZlibDecoder::new(std::io::Cursor::new(&side_band_merged[current_offset..]));
            let mut delta_bytes = Vec::new();
            decoder.read_to_end(&mut delta_bytes).unwrap();
            current_offset += decoder.total_in() as usize;

            let mut base_pos = base_object_start;
            let mut base_first = side_band_merged[base_pos];
            base_pos += 1;
            while base_first & 0x80 != 0 {
                base_first = side_band_merged[base_pos];
                base_pos += 1;
            }
            
            let base_key = current_object_start - delta_offset as usize;
            let (base_type, base_bytes) = pack_objects.get(&base_key).expect("base object not found in map");

            let reconstructed = apply_delta(&base_bytes, &delta_bytes);
            let header = format!("{} {}", base_type, reconstructed.len());
            let data = [header.as_bytes(), &[0], &reconstructed].concat();
            let _ = write_git_object(&clone_dir, &data);
            pack_objects.insert(current_object_start, (base_type.to_string(), reconstructed.clone()));
            continue;
        }
        if object_type == "ref_delta" {
            continue;  // Not implemented
        }
        
        let mut object = ZlibDecoder::new(std::io::Cursor::new(&side_band_merged[current_offset..]));
        let mut object_bytes = Vec::new();
        object.read_to_end(&mut object_bytes).unwrap();
        let position = object.total_in();
        current_offset += position as usize;

        if !std::path::Path::new(&clone_dir).exists() {
            std::fs::create_dir(&clone_dir).unwrap();
        }
        
        let header = format!("{} {}", object_type, object_bytes.len());
        let data = [header.as_bytes(), &[0], &object_bytes].concat();
        let _ = write_git_object(&clone_dir, &data);
        
        pack_objects.insert(current_object_start, (object_type.to_string(), object_bytes.clone()));
    }
    // Write .git/HEAD
    let head_file = format!("{}/.git/HEAD", &clone_dir);
    let mut f = fs::File::create(head_file).unwrap();
    f.write_all(b"ref: refs/heads/main\n").unwrap();
    // Write .git/refs/heads/main
    let ref_dir = format!("{}/.git/refs/heads/", &clone_dir);
    fs::create_dir_all(&ref_dir).unwrap();
    let file = format!("{}/main", ref_dir);
    let mut f = fs::File::create(file).unwrap();
    f.write_all(&hash).unwrap();
    
    let object_id = str::from_utf8(&hash).unwrap();
    let path = format!("{}/.git/objects/{}/{}", &clone_dir, &object_id[..2], &object_id[2..]);
    let mut d = ZlibDecoder::new(fs::File::open(path).unwrap());
    let mut bytes = Vec::new();
    d.read_to_end(&mut bytes).unwrap();
    
    if let Some(null_pos) = bytes.iter().position(|&b| b == 0) {
        let content = &bytes[null_pos + 1..];
        let content_str = String::from_utf8_lossy(content).into_owned();
        let split: Vec<&str> = content_str.split_whitespace().collect();

        write_dir_to_disk(&clone_dir, &clone_dir, &split[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup() {
        //
    }

    #[test]
    fn test_command_init_ok() {
        let dir = tempdir().unwrap();
        let path = dir.path();
    
        let _ = command_init(path.to_str().unwrap());
    
        assert!(path.join(".git").exists());
        assert!(path.join(".git/objects").exists());
        assert!(path.join(".git/refs").exists());
    
        let head = fs::read_to_string(path.join(".git/HEAD")).unwrap();
        assert_eq!(head, "ref: refs/heads/main\n");
    }

    #[test]
    fn test_command_init_fails_when_repo_exists() {
        let dir = tempdir().unwrap();
        let path = dir.path(); 
        let _ = command_init(path.to_str().unwrap());
    
        assert!(command_init(path.to_str().unwrap()).is_err());
    }

    #[test]
    fn test_command_cat_file_ok() {
        let dir = tempdir().unwrap();
        let path = dir.path();
        let file_path = path.join("test.txt");
        
        fs::write(&file_path, "hello\n").unwrap();
        
        let objet_id = command_hash_object(&file_path.to_str().unwrap(), path.to_str().unwrap());
        let content = command_cat_file(&objet_id, path.to_str().unwrap());
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn test_command_hash_object_ok() {
        let dir = tempdir().unwrap();
        let path = dir.path();
        let file_path = path.join("test.txt");
        let content = b"hello\n";
        
        fs::write(&file_path, content).unwrap();
        let hex_hash = command_hash_object(&file_path.to_str().unwrap(), path.to_str().unwrap());

        // Independently compute the expected git blob hash
        let blob = [format!("blob {}\0", content.len()).as_bytes(), content].concat();
        let expected = hex::encode(Sha1::digest(&blob));
        assert_eq!(hex_hash, expected);
    }

    #[test]
    fn test_command_ls_tree_name_only_true() {
        let dir = tempdir().unwrap();
        let path = dir.path();
        fs::create_dir(path.join("a")).unwrap();
        fs::create_dir(path.join("b")).unwrap();

        let file_path = path.join("test.txt");
        fs::write(&file_path, "hello\n").unwrap();

        let tree_id = command_write_tree(&path, path.to_str().unwrap());
        
        let output = command_ls_tree(&tree_id, &true, path.to_str().unwrap());
        let expected = vec![String::from("a"), String::from("b"), String::from("test.txt")];
        assert_eq!(output, expected);
    }

    // This one is especially silly but it never the less helps me to understand the flows
    #[test]
    fn test_command_ls_tree_name_only_false() {
        let dir = tempdir().unwrap();
        let path = dir.path();
        fs::create_dir(path.join("a")).unwrap();
        fs::create_dir(path.join("b")).unwrap();

        let file_path = path.join("test.txt");
        fs::write(&file_path, "hello\n").unwrap();

        let tree_id = command_write_tree(&path, path.to_str().unwrap());

        let bytes = read_git_object(path.to_str().unwrap(), &tree_id);
        let records = parse_tree_entries(&bytes);

        let expected: Vec<String> = records.into_iter().map(|r| r.mode + " - " + &r.name + " - " + &r.hash).collect();
        let output = command_ls_tree(&tree_id, &false, path.to_str().unwrap());
        assert_eq!(output, expected);
    }

    #[test]
    fn test_command_write_tree() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        let file_path = path.join("test.txt");
        fs::write(&file_path, "hello\n").unwrap();

        let mut contents = Vec::new();
        let header = format!("blob {}", "hello\n".len());
        let data = [header.as_bytes(), &[0], "hello\n".as_bytes()].concat();
        let hash = Sha1::digest(&data);
        let header = format!("100644 test.txt\0").into_bytes();
        contents.extend_from_slice(&header);
        contents.extend_from_slice(&hash.to_vec());

        let header = format!("tree {}\0", contents.len());
        let mut data = header.into_bytes();
        data.extend_from_slice(&contents);

        let hash = command_write_tree(&path, path.to_str().unwrap());
        let expected = Sha1::digest(&data);
        assert_eq!(hash, format!("{:x}", expected));
    }
}
