use testcontainers::{Container, ContainerClient, Docker, Image, WaitForMessage};
use web3_client::Web3Client;

pub struct GanacheCli {
    tag: String,
    arguments: GanacheCliArgs,
}

#[derive(Clone)]
pub struct GanacheCliArgs {
    pub network_id: u32,
    pub number_of_accounts: u32,
    pub mnemonic: String,
}

impl Default for GanacheCliArgs {
    fn default() -> Self {
        GanacheCliArgs {
            network_id: 42,
            number_of_accounts: 7,
            mnemonic: "supersecure".to_string(),
        }
    }
}

impl IntoIterator for GanacheCliArgs {
    type Item = String;
    type IntoIter = ::std::vec::IntoIter<String>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        let mut args = Vec::new();

        if !self.mnemonic.is_empty() {
            args.push("-m".to_string());
            args.push(format!("{}", self.mnemonic));
        }

        args.push("-a".to_string());
        args.push(format!("{}", self.number_of_accounts));
        args.push("-i".to_string());
        args.push(format!("{}", self.network_id));

        args.into_iter()
    }
}

impl Image for GanacheCli {
    type Args = GanacheCliArgs;

    fn descriptor(&self) -> String {
        format!("trufflesuite/ganache-cli:{}", self.tag)
    }

    fn wait_until_ready<D: Docker>(&self, container: &Container<D>) {
        container
            .logs()
            .wait_for_message("Listening on localhost:")
            .unwrap();
    }

    fn args(&self) -> <Self as Image>::Args {
        self.arguments.clone()
    }

    fn with_args(self, arguments: <Self as Image>::Args) -> Self {
        GanacheCli { arguments, ..self }
    }

    fn new(tag: &str) -> Self {
        GanacheCli {
            tag: tag.to_string(),
            arguments: GanacheCliArgs::default(),
        }
    }
}

impl ContainerClient<Web3Client> for GanacheCli {
    fn new_container_client<D: Docker>(container: &Container<D>, _image: &Self) -> Web3Client {
        let host_port = container.get_host_port(8545).unwrap();

        let url = format!("http://localhost:{}", host_port);

        Web3Client::new(&url).unwrap()
    }
}

impl Default for GanacheCli {
    fn default() -> Self {
        GanacheCli::new("v6.1.3")
    }
}
