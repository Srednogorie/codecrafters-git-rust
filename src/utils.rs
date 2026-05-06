use crate::structs::TreeEntry;

use std::fs;
use std::io::prelude::*;
use flate2::read::ZlibDecoder;
use sha1::{Sha1, Digest};
use flate2::write::ZlibEncoder;


pub fn write_git_object(base_dir: &str, data: &[u8]) -> String {
    let hash = Sha1::digest(&data);
    let hex_hash = format!("{:x}", hash);
    let dir = format!("{}/.git/objects/{}", base_dir, &hex_hash[..2]);
    fs::create_dir_all(&dir).unwrap();
    let file = format!("{}/{}", dir, &hex_hash[2..]);
    let mut f = fs::File::create(file).unwrap();
    let mut compressed = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    compressed.write_all(&data).unwrap();
    let compressed_bytes = compressed.finish().unwrap();
    f.write_all(&compressed_bytes).unwrap();
    hex_hash
}

pub fn read_git_object(base_dir: &str, hash: &str) -> Vec<u8> {
    let path = format!("{}/.git/objects/{}/{}", base_dir, &hash[..2], &hash[2..]);
    
    let mut d = ZlibDecoder::new(fs::File::open(path).unwrap());
    let mut bytes = Vec::new();
    d.read_to_end(&mut bytes).unwrap();
    bytes
}

pub fn parse_tree_entries(data: &[u8]) -> Vec<TreeEntry> {
    let mut line = Vec::new();
    let mut header_vec = Vec::new();
    let mut header = false;
    let mut current_bytes = Vec::new();
    let mut reading_bytes = false;
    let mut record = String::new();
    let mut records: Vec<TreeEntry> = Vec::new();
    for ch in data {
        if !header && *ch != b'\0' {
            header_vec.push(ch);
            continue;
        }
        if !header && *ch == b'\0' {
            header = true;
            continue;
        }
        // <mode> <filename>\0<20 bytes>
        if *ch == b' ' && !reading_bytes {
            let mode = &line;
            record.push_str(&format!("{:0>6} ", String::from_utf8_lossy(&mode)));
            line.clear();
            continue;
        } else if *ch == b'\0' && !reading_bytes {
            let filename = &line;
            record.push_str(&format!("{} ", String::from_utf8_lossy(&filename)));
            line.clear();
            reading_bytes = true;
            continue;
        } else {
            if reading_bytes && current_bytes.len() != 20 {
                current_bytes.push(*ch);
            } else {
                if current_bytes.len() == 20 {
                    let hex_hash: String = current_bytes.iter().map(|b| format!("{:02x}", b)).collect();
                    record.push_str(&hex_hash);
                    current_bytes.clear();
                    reading_bytes = false;
                    
                    // records.push(record.clone());
                    let split_record = record.split(' ').collect::<Vec<&str>>();
                    records.push(TreeEntry {
                        mode: split_record[0].to_string(),
                        name: split_record[1].to_string(),
                        hash: split_record[2].to_string(),
                    });
                    record.clear();
                }
                line.push(*ch);
            }
        }
    }
    // Get the last record
    let hex_hash: String = current_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    record.push_str(&hex_hash);
    let split_record = record.split(' ').collect::<Vec<&str>>();
    records.push(TreeEntry {
        mode: split_record[0].to_string(),
        name: split_record[1].to_string(),
        hash: split_record[2].to_string(),
    });
    records
}

pub fn get_file_blob<R: Read>(mut reader: R) -> Vec<u8> {
    let mut contents = Vec::new();
    reader.read_to_end(&mut contents).unwrap();
    let header = format!("blob {}", contents.len());
    let data = [header.as_bytes(), &[0], &contents].concat();
    data
}

pub fn apply_delta(base: &[u8], delta: &[u8]) -> Vec<u8> {
    let mut pos = 0;

    // Read variable length integer
    let read_varint = |pos: &mut usize| -> usize {
        let mut result = 0usize;
        let mut shift = 0;
        loop {
            let b = delta[*pos];
            *pos += 1;
            result |= ((b & 0x7f) as usize) << shift;
            shift += 7;
            if b & 0x80 == 0 { break; }
        }
        result
    };

    let _source_size = read_varint(&mut pos);
    let target_size = read_varint(&mut pos);
    let mut output = Vec::with_capacity(target_size);

    while pos < delta.len() {
        let cmd = delta[pos];
        pos += 1;
        if cmd & 0x80 != 0 {
            // Copy instruction
            let mut copy_offset = 0usize;
            let mut copy_size = 0usize;
            if cmd & 0x01 != 0 { copy_offset |= delta[pos] as usize; pos += 1; }
            if cmd & 0x02 != 0 { copy_offset |= (delta[pos] as usize) << 8; pos += 1; }
            if cmd & 0x04 != 0 { copy_offset |= (delta[pos] as usize) << 16; pos += 1; }
            if cmd & 0x08 != 0 { copy_offset |= (delta[pos] as usize) << 24; pos += 1; }
            if cmd & 0x10 != 0 { copy_size |= delta[pos] as usize; pos += 1; }
            if cmd & 0x20 != 0 { copy_size |= (delta[pos] as usize) << 8; pos += 1; }
            if cmd & 0x40 != 0 { copy_size |= (delta[pos] as usize) << 16; pos += 1; }
            if copy_size == 0 { copy_size = 0x10000; }
            output.extend_from_slice(&base[copy_offset..copy_offset + copy_size]);
        } else {
            // Insert instruction
            let insert_size = (cmd & 0x7f) as usize;
            output.extend_from_slice(&delta[pos..pos + insert_size]);
            pos += insert_size;
        }
    }
    output
}

fn write_file_to_disk(clone_dir: &str, current_path: &str, pathname: &str, hash: &str) {
    let file_path = format!("{}/{}", current_path, pathname);
    let mut f = fs::File::create(&file_path).unwrap();

    let bytes = read_git_object(clone_dir, hash);    
    if let Some(null_pos) = bytes.iter().position(|&b| b == 0) {
        let content = &bytes[null_pos + 1..];
        f.write_all(content).unwrap();
    }
}

pub fn write_dir_to_disk(clone_dir: &str, current_path: &str, hash_str: &str) {
    let bytes = read_git_object(clone_dir, hash_str);
    
    let records = parse_tree_entries(&bytes);
    
    for record in records {        
        if record.mode == "100644" {
            write_file_to_disk(clone_dir, current_path, &record.name, &record.hash);
        } else if record.mode == "040000" {
            let new_path = format!("{}/{}", current_path, record.name);
            fs::create_dir(&new_path).unwrap();
            write_dir_to_disk(clone_dir, &new_path, &record.hash);
        }
    }
}

/*

b"001e# service=git-upload-pack\n0000015926f9dc6eafcd97c3deff2018f69471c18f7704c7 HEAD\0multi_ack thin-pack side-band side-band-64k ofs-delta shallow deepen-since deepen-not deepen-relative no-progress include-tag multi_ack_detailed allow-tip-sha1-in-want allow-reachable-sha1-in-want no-done symref=HEAD:refs/heads/main filter object-format=sha1 agent=git/github-f8bdfd365d97-Linux\n003d26f9dc6eafcd97c3deff2018f69471c18f7704c7 refs/heads/main\n003f26f9dc6eafcd97c3deff2018f69471c18f7704c7 refs/heads/master\n0000"

Splitting the response from git-upload-pack into lines.
The first 4 bytes of each line are the length in hex so we parse that into an integer to get the line data - 
from_str_radix with base 16.

*/

pub fn read_pkt_lines(mut data: &[u8]) -> Vec<Vec<u8>> {
    let mut lines = Vec::new();
    while data.len() >= 4 {
        let len = usize::from_str_radix(std::str::from_utf8(&data[..4]).unwrap(), 16).unwrap();
        if len == 0 {
            data = &data[4..];
            continue;
        }

        let line = &data[4..len];
        lines.push(line.to_vec());

        data = &data[len..];
    }
    lines
}