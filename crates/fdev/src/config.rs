use std::{fmt::Display, path::PathBuf};

use crate::{commands::PutType, wasm_runtime::ExecutorConfig};
use clap::ValueEnum;
use freenet::dev_tool::OperationMode;
use semver::Version;

#[derive(clap::Parser, Clone)]
#[clap(name = "Freenet Development Tool")]
#[clap(author = "The Freenet Project Inc.")]
#[clap(version = "0.0.6")]
pub struct Config {
    #[clap(subcommand)]
    pub sub_command: SubCommand,
    #[clap(flatten)]
    pub additional: BaseConfig,
}

#[derive(clap::Parser, Clone)]
pub struct BaseConfig {
    /// Overrides the default data directory where Freenet contract files are stored.
    #[arg(long)]
    pub(crate) contract_data_dir: Option<PathBuf>,
    /// Overrides the default data directory where Freenet delegate files are stored.
    #[arg(long)]
    pub(crate) delegate_data_dir: Option<PathBuf>,
    /// Overrides the default data directory where Freenet secret files are stored.
    #[arg(long)]
    pub(crate) secret_data_dir: Option<PathBuf>,
    /// Node operation mode.
    #[arg(value_enum, default_value_t=OperationMode::Local)]
    pub mode: OperationMode,
}

#[derive(clap::Subcommand, Clone)]
pub enum SubCommand {
    New(NewPackageConfig),
    Build(BuildToolConfig),
    Inspect(crate::inspect::InspectConfig),
    Publish(PutConfig),
    WasmRuntime(ExecutorConfig),
    Execute(RunCliConfig),
    Test(crate::testing::TestConfig),
}

impl SubCommand {
    pub fn is_child(&self) -> bool {
        if let SubCommand::Test(config) = self {
            if let crate::testing::TestMode::MultiProcess(config) = &config.command {
                return matches!(config.mode, crate::testing::Process::Child);
            }
        }
        false
    }
}

/// Core CLI tool for interacting with the Freenet local node.
///
/// This tool allows the execution of commands against the local node
/// and is intended to be used for development and automated workflows.
#[derive(clap::Parser, Clone)]
pub struct RunCliConfig {
    /// Command to execute.
    #[clap(subcommand)]
    pub command: NodeCommand,
}

#[derive(clap::Subcommand, Clone)]
pub enum NodeCommand {
    Put(PutConfig),
    Update(UpdateConfig),
}

/// Updates a contract in the network.
#[derive(clap::Parser, Clone)]
pub struct UpdateConfig {
    /// Contract id of the contract being updated in Base58 format.
    pub(crate) key: String,
    /// A path to the update/delta being pushed to the contract.
    pub(crate) delta: PathBuf,
    /// Whether this contract will be updated in the network or is just a dry run
    /// to be executed in local mode only. By default puts are performed in local.
    pub(crate) release: bool,
}

/// Publishes a new contract or delegate to the network.
// todo: make some of this options exclusive depending on the value of `package_type`
#[derive(clap::Parser, Clone)]
pub struct PutConfig {
    /// A path to the compiled WASM code file. This must be a valid packaged contract or component,
    /// (built using the `fdev` tool). Not an arbitrary WASM file.
    #[arg(long)]
    pub(crate) code: PathBuf,
    /// A path to the file parameters for the contract/delegate. If not specified, will be published
    /// with empty parameters.
    #[arg(long)]
    pub(crate) parameters: Option<PathBuf>,
    /// Whether this contract will be released into the network or is just a dry run
    /// to be executed in local mode only. By default puts are performed in local.
    #[arg(long)]
    pub(crate) release: bool,
    /// Type of put to perform.
    #[clap(subcommand)]
    pub(crate) package_type: PutType,
}

/// Builds and packages a contract or delegate.
///
/// This tool will build the WASM contract or delegate and publish it to the network.
#[derive(clap::Parser, Clone, Debug)]
pub struct BuildToolConfig {
    /// Compile the contract or delegate with specific features.
    #[arg(long)]
    pub(crate) features: Option<String>,

    /// Compile the contract or delegate with a specific API version.
    #[arg(long, value_parser = parse_version, default_value_t=Version::new(0, 0, 1))]
    pub(crate) version: Version,

    /// Output object type.
    #[arg(long, value_enum, default_value_t=PackageType::default())]
    pub(crate) package_type: PackageType,

    /// Compile in debug mode instead of release.
    #[arg(long)]
    pub(crate) debug: bool,
}

#[derive(Default, Debug, Clone, Copy, ValueEnum)]
pub(crate) enum PackageType {
    #[default]
    Contract,
    Delegate,
}

impl PackageType {
    pub fn feature(&self) -> &'static str {
        match self {
            PackageType::Contract => "freenet-main-contract",
            PackageType::Delegate => "freenet-main-delegate",
        }
    }
}

impl Display for PackageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageType::Contract => write!(f, "contract"),
            PackageType::Delegate => write!(f, "delegate"),
        }
    }
}

impl Default for BuildToolConfig {
    fn default() -> Self {
        Self {
            features: None,
            version: Version::new(0, 0, 1),
            package_type: PackageType::default(),
            debug: false,
        }
    }
}

fn parse_version(src: &str) -> Result<Version, String> {
    Version::parse(src).map_err(|e| e.to_string())
}

/// Create a new Freenet contract and/or app.
#[derive(clap::Parser, Clone)]
pub struct NewPackageConfig {
    #[arg(id = "type", value_enum)]
    pub(crate) kind: ContractKind,
}

#[derive(clap::ValueEnum, Clone)]
pub(crate) enum ContractKind {
    /// A web app container contract.
    WebApp,
    /// An standard contract.
    Contract,
}
