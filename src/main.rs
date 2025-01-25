use git2::{DiffOptions, Repository};
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, BufWriter, Write},
    ops::Add,
    path::{Path, PathBuf},
    str,
};
use std::{env, fs::OpenOptions};

mod error;
use error::RTrimError;


#[cfg(windows)]
const LINE_ENDING: &[u8] = b"\r\n";
#[cfg(not(windows))]
const LINE_ENDING: &[u8] = b"\n";


fn path_combine<T>(path1: T, path2: T) -> PathBuf
where
    T: AsRef<Path>,
    PathBuf: From<T>,
    T: Into<PathBuf>,
{
    let mut buf = PathBuf::from(path1);
    buf.push(path2);
    buf
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
/*
    ADR: git2 only returns LF, no CRLFs => no need to check for CRLF
*/
fn trailing_whitespaces(s: &str) -> bool {
    s.ends_with(' ')
    || s.ends_with('\t')
    || s.ends_with(" \n")
    || s.ends_with("\t\n")
}

fn get_staged_lines_with_trailing_spaces(
    repo: &Repository,
    path_filters: &[String]
) -> Result<HashMap<String, VecDeque<u32>>, RTrimError> {
    let mut result: HashMap<String, VecDeque<u32>> = HashMap::new();

    //get head_tree
    let head_tree = match repo.head() {
        Ok(r) => Option::Some(r.peel_to_tree()?),
        Err(_) => Option::None,
    };

    //get index
    let index = Option::Some(repo.index()?);

    let mut diff_options = DiffOptions::new();

    for path_filter in path_filters {
        diff_options.pathspec(path_filter);
    }

    //get diff
    let diff_result = repo.diff_tree_to_index(head_tree.as_ref(), index.as_ref(), Some(&mut diff_options))?;

    //iterate over the diff_result and put lines with trailing spaces in the result
    diff_result.print(git2::DiffFormat::Patch, |d, _, diff_line| -> bool {
        if let Some(line_no) = diff_line.new_lineno() {
            let raw_line = diff_line.content();

            if let Ok(line) = str::from_utf8(raw_line) {
                if trailing_whitespaces(line) {
                    let file_path = PathBuf::from(d.new_file().path().unwrap());
                    let file_path_str = String::from(file_path.to_str().unwrap());

                    match result.get_mut(&file_path_str) {
                        Some(l) => {
                            l.push_back(line_no);
                        }
                        None => {
                            let mut queue: VecDeque<u32> = VecDeque::new();
                            queue.push_back(line_no);
                            result.insert(file_path_str, queue);
                        }
                    }
                }
            }
        }

        true
    })?;

    Ok(result)
}

fn rtrim_files(dir: &Path, files: &HashMap<String, VecDeque<u32>>) -> Result<(), std::io::Error> {
    for (file_name, l) in files {
        let mut lines = l.clone();

        //setup file reader
        let file_path = path_combine(dir, file_name.as_ref());
        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);

        //setup file writer
        let new_file_suffix = calculate_hash(file_name).to_string();
        let new_file_name = String::from(file_name).add(&new_file_suffix);
        let new_file_path = path_combine(dir, new_file_name.as_ref());

        let new_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&new_file_path)?;

        let mut writer = BufWriter::new(new_file);

        let mut line_no = 1;
        for value in reader.lines() {
            //wrap result
            let line: String = value?;
            let mut line_to_write: &str = line.as_str();

            if let Some(l) = lines.front() {
                if line_no == *l {
                    line_to_write = line.trim_end();

                    _ = lines.pop_front();
                }
            }

            writer.write_all(line_to_write.as_bytes())?;
            writer.write_all(LINE_ENDING)?;

            line_no += 1;
        }

        writer.flush()?;

        std::fs::rename(&new_file_path, &file_path)?;
    }

    Ok(())
}

fn add_files<'a, T>(repo: &Repository, files: T) -> Result<(), git2::Error>
where
    T: Iterator<Item = &'a String>,
{
    let mut index = repo.index()?;

    for file in files {
        index.add_path(PathBuf::from(file).as_path())?;
    }

    index.write()?;

    Ok(())
}

fn run(args: &Vec<String>) -> Result<(), RTrimError> {
    let working_dir = env::current_dir()?;
    let repo = Repository::discover(&working_dir)?;
    let path_filters = &args[1..args.len()];

    let repo_workdir = if let Some(repo_workdir) = repo.workdir() {
        repo_workdir
    }else {
        &working_dir
    };

    let files = get_staged_lines_with_trailing_spaces(&repo, &path_filters)?;

    rtrim_files(repo_workdir, &files)?;
    add_files(&repo, files.keys())?;

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match run(&args) {
        Ok(_) => {}
        Err(err) => {

            // ToDo can I avoid this allocation?
            let error_message = match err {
                RTrimError::Git(ge) => {
                    ge.message().to_string()
                },

                RTrimError::Io(ioe) => {
                    ioe.to_string()
                }
            };

            eprintln!("error {}", error_message);
        }
    }
}
