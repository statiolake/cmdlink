use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, io};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {}", 0)]
    IOError(io::Error),
    #[error("metadata not found for {}", path.display())]
    MetadataNotFound { path: PathBuf },
    #[error("invalid syntax at {}:{}: {}", metadata_path.display(), idx, line)]
    InvalidMetadataSyntax {
        metadata_path: PathBuf,
        idx: usize,
        line: String,
    },
    #[error("no `{}` specified in {}", prop_name, metadata_path.display())]
    PropertyNotSpecified {
        metadata_path: PathBuf,
        prop_name: String,
    },
    #[error("type of `{}` is wrong in {}", prop_name, metadata_path.display())]
    WrongPropertyType {
        metadata_path: PathBuf,
        prop_name: String,
    },
    #[error("invalid value {} in {}: {}", prop_name, metadata_path.display(), reason)]
    InvalidValue {
        metadata_path: PathBuf,
        prop_name: String,
        reason: String,
    },
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IOError(err)
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Metadata {
    program_path: PathBuf,
    args: Vec<ArgKind>,
}

enum ArgKind {
    Literal(String),
    Arg(usize),
    Args,
}

struct MetadataTable {
    metadata_path: PathBuf,
    table: HashMap<String, MetadataTableValue>,
}

enum MetadataTableValue {
    String(String),
    Array(Vec<String>),
}

impl MetadataTable {
    pub fn parse(metadata_path: &Path) -> Result<MetadataTable> {
        let string = read_to_string(&metadata_path)?;
        let mut table = HashMap::new();
        for (idx, line) in string.lines().enumerate() {
            let [key, value]: [&str; 2] = line
                .splitn(2, '=')
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| Error::InvalidMetadataSyntax {
                    metadata_path: metadata_path.to_path_buf(),
                    idx,
                    line: line.to_string(),
                })?;
            let key = key.trim().to_string();
            let value = value.trim();

            let value = if let Some(value) = unwrap_wrapped(value, '[', ']') {
                // array
                let values = value
                    .split(',')
                    .map(|value| value.trim())
                    .map(|value| unwrap_wrapped(value, '"', '"').unwrap_or(value))
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                MetadataTableValue::Array(values)
            } else {
                // string
                let value = unwrap_wrapped(value, '"', '"').unwrap_or(value);
                MetadataTableValue::String(value.to_string())
            };

            table.insert(key, value);
        }

        Ok(MetadataTable {
            metadata_path: metadata_path.to_path_buf(),
            table,
        })
    }

    pub fn get_string(&self, key: &str) -> Result<&str> {
        self.get(key).and_then(|value| match value {
            MetadataTableValue::String(s) => Ok(&**s),
            MetadataTableValue::Array(_) => Err(Error::WrongPropertyType {
                metadata_path: self.metadata_path.clone(),
                prop_name: key.to_string(),
            }),
        })
    }

    pub fn get_array(&self, key: &str) -> Result<&[String]> {
        self.get(key).and_then(|value| match value {
            MetadataTableValue::String(_) => Err(Error::WrongPropertyType {
                metadata_path: self.metadata_path.clone(),
                prop_name: key.to_string(),
            }),
            MetadataTableValue::Array(a) => Ok(&a[..]),
        })
    }

    pub fn get(&self, key: &str) -> Result<&MetadataTableValue> {
        self.table
            .get(key)
            .ok_or_else(|| Error::PropertyNotSpecified {
                metadata_path: self.metadata_path.clone(),
                prop_name: key.to_string(),
            })
    }
}

fn unwrap_wrapped(s: &str, left: char, right: char) -> Option<&str> {
    if s.starts_with(left) && s.ends_with(right) {
        Some(&s[1..s.len() - 1])
    } else {
        None
    }
}

impl Metadata {
    pub fn load(exe: &Path) -> Result<Metadata> {
        let metadata_path = exe.with_extension("meta");
        if !metadata_path.exists() {
            return Err(Error::MetadataNotFound {
                path: metadata_path,
            });
        }

        Metadata::parse(metadata_path)
    }

    pub fn parse(metadata_path: PathBuf) -> Result<Metadata> {
        let table = MetadataTable::parse(&metadata_path)?;
        let program_path = table.get_string("path").map(PathBuf::from)?;
        let args = table
            .get_array("args")?
            .iter()
            .map(|arg| {
                if arg == "%*" {
                    Ok(ArgKind::Args)
                } else if let Some(idx) = arg.strip_prefix('%') {
                    idx.parse::<usize>()
                        .map_err(|e| Error::InvalidValue {
                            metadata_path: metadata_path.clone(),
                            prop_name: "args".to_string(),
                            reason: format!("invalid arg index: {}: {}", &arg[1..], e),
                        })
                        .map(ArgKind::Arg)
                } else {
                    Ok(ArgKind::Literal(arg.to_string()))
                }
            })
            .collect::<Result<_>>()?;

        Ok(Metadata { program_path, args })
    }
}

pub fn run(metadata: &Metadata) -> Result<i32> {
    let args = env::args().collect::<Vec<_>>();
    let program_path = &metadata.program_path;
    let res = Command::new(program_path)
        .args(metadata.args.iter().flat_map(|arg| match arg {
            ArgKind::Literal(lit) => vec![lit.clone()],
            ArgKind::Arg(idx) => vec![args[*idx].clone()],
            ArgKind::Args => Vec::from(&args[1..]),
        }))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?
        .wait()?;
    Ok(res.code().unwrap_or(1))
}

pub fn run_for_current_exe() -> Result<i32> {
    let exe = env::current_exe()?;
    if !exe.exists() {
        panic!("internal error: link execuable does not exist")
    }

    let metadata = Metadata::load(&exe)?;
    run(&metadata)
}
