use std::env;
use std::fmt;
use std::fs::{read_to_string, File};
use std::io;
use std::num::ParseIntError;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use std::sync::mpsc::{channel, Sender};
use std::{thread, time};

use ozelot::clientbound::*;
use ozelot::{mojang, serverbound, Client};
use rpassword;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use structopt::StructOpt;

fn main() {
    let opt = Opt::from_args();
    let mut client = if !opt.offline {
        let auth = authenticate(opt.account, opt.profile);
        println!("Authentication successful!, connecting to server...");
        match Client::connect_authenticated(&opt.server.host, opt.server.port, &auth) {
            Ok(x) => x,
            Err(e) => {
                println!("Error connecting to {}: {:?}", opt.server, e);
                exit(1);
            }
        }
    } else {
        println!("Connecting to server...");
        match Client::connect_unauthenticated(&opt.server.host, opt.server.port, &opt.account) {
            Ok(x) => x,
            Err(e) => {
                println!(
                    "Error connecting unauthenticated to {}: {:?}",
                    opt.server, e
                );
                exit(1);
            }
        }
    };

    println!("Connected!");

    let (tx, rx) = channel();
    thread::spawn(move || {
        read_stdin(tx);
    });

    'main: loop {
        let packets = client.read().unwrap();
        let timeout = if packets.is_empty() {
            time::Duration::from_millis(50)
        } else {
            time::Duration::default()
        };
        for packet in packets {
            match packet {
                ClientboundPacket::JoinGame(_) => {
                    let settings = serverbound::ClientSettings::new(get_locale(), 2, 0, true, 0, 0);
                    client.send(settings).unwrap();
                }
                ClientboundPacket::PlayDisconnect(ref p) => {
                    println!("Got disconnect packet, exiting ...");
                    println!("Reason: {}", p.get_reason());
                    break 'main;
                }
                ClientboundPacket::ChatMessage(ref p) => {
                    let msg = p.get_chat();
                    println!("{}", msg);
                }
                _ => (),
            }
        }

        if let Ok(msg) = rx.recv_timeout(timeout) {
            let msg = msg.trim_end().to_string();
            let chat = serverbound::ChatMessage::new(msg);
            client.send(chat).unwrap();
        }
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    /// Mojang account
    #[structopt(short = "u", long)]
    account: String,
    /// Server address
    #[structopt(short = "s", long)]
    server: ServerAddress,
    /// Offline mode
    offline: bool,
    /// Path to profile
    #[structopt(short = "c", long)]
    profile: Option<String>,
    /// Enable auto-Reconnect
    #[structopt(short = "r", long)]
    reconnect: bool,
}

#[derive(Debug, PartialEq)]
struct ServerAddress {
    host: String,
    port: u16,
}

impl FromStr for ServerAddress {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.rfind(':').is_some() {
            let mut iter = s.rsplitn(2, ':');
            let (port, host) = (
                u16::from_str(iter.next().unwrap())?,
                iter.next().unwrap().to_owned(),
            );
            Ok(ServerAddress { host, port })
        } else {
            Ok(ServerAddress {
                host: s.to_owned(),
                port: 25565,
            })
        }
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

fn authenticate(account: String, profile: Option<String>) -> mojang::AuthenticationResponse {
    let ask_passwd = || {
        let password = rpassword::prompt_password_stdout("Enter password: ").unwrap();
        mojang::Authenticate::new(account, password)
            .perform()
            .unwrap()
    };
    if let Some(config_path) = profile {
        let config_path = Path::new(&config_path);
        let auth = if config_path.exists() {
            println!("Reading profile...");
            let config = read_to_string(&config_path).expect("failed to read profile");
            let config: AuthProfile = serde_json::from_str(&config).expect("");
            let validate = mojang::AuthenticateValidate::new(
                config.access_token.clone(),
                config.client_token.clone(),
            )
            .perform()
            .is_ok();
            if validate {
                println!("Valid profile!");
                config.into()
            } else {
                println!("Outdated profile, refreshing..");
                mojang::AuthenticateRefresh::new(
                    config.access_token,
                    config.client_token.expect(""),
                    true,
                )
                .perform()
                .unwrap_or_else(|e| {
                    println!("Failed to refresh profile({}), please re-login.", e);
                    ask_passwd()
                })
            }
        } else {
            println!("Profile doesn't exists, please login.");
            ask_passwd()
        };
        let file = File::create(&config_path).expect("");
        serde_json::to_writer_pretty(&file, &AuthProfile::from(auth.clone())).expect("");
        auth
    } else {
        ask_passwd()
    }
}

fn get_locale() -> String {
    match env::var("LANG") {
        Ok(ref lang) if lang != "C" => lang.split('.').next().unwrap().to_owned(),
        _ => "en_US".to_owned(),
    }
}

fn read_stdin(tx: Sender<String>) {
    loop {
        let mut tmp = String::new();
        let _: usize = io::stdin().read_line(&mut tmp).unwrap();
        tx.send(tmp).unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AuthProfile {
    access_token: String,
    client_token: Option<String>,
    available_profiles: Option<Vec<NameUUID>>,
    selected_profile: NameUUID,
}

impl From<AuthProfile> for mojang::AuthenticationResponse {
    fn from(profile: AuthProfile) -> mojang::AuthenticationResponse {
        mojang::AuthenticationResponse {
            accessToken: profile.access_token,
            clientToken: profile.client_token,
            availableProfiles: profile
                .available_profiles
                .map(|x| x.into_iter().map(Into::into).collect()),
            selectedProfile: profile.selected_profile.into(),
        }
    }
}

impl From<mojang::AuthenticationResponse> for AuthProfile {
    fn from(profile: mojang::AuthenticationResponse) -> AuthProfile {
        AuthProfile {
            access_token: profile.accessToken,
            client_token: profile.clientToken,
            available_profiles: profile
                .availableProfiles
                .map(|x| x.into_iter().map(Into::into).collect()),
            selected_profile: profile.selectedProfile.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NameUUID {
    /// The uuid in hex without dashes
    id: String,
    /// Name of the player at the present point in time
    name: String,
    #[serde(default)]
    legacy: bool,
    #[serde(default)]
    demo: bool,
}

impl From<NameUUID> for mojang::NameUUID {
    fn from(uuid: NameUUID) -> mojang::NameUUID {
        mojang::NameUUID {
            id: uuid.id,
            name: uuid.name,
            legacy: uuid.legacy,
            demo: uuid.demo,
        }
    }
}

impl From<mojang::NameUUID> for NameUUID {
    fn from(uuid: mojang::NameUUID) -> NameUUID {
        NameUUID {
            id: uuid.id,
            name: uuid.name,
            legacy: uuid.legacy,
            demo: uuid.demo,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn server_address() {
        let r = ServerAddress::from_str("127.0.0.1:25566").unwrap();
        assert_eq!(
            r,
            ServerAddress {
                host: String::from("127.0.0.1"),
                port: 25566,
            }
        );
        assert_eq!(format!("{}", r), "127.0.0.1:25566");
    }

    #[test]
    fn server_address_default_port() {
        let r = ServerAddress::from_str("127.0.0.1").unwrap();
        assert_eq!(
            r,
            ServerAddress {
                host: "127.0.0.1".into(),
                port: 25565
            }
        );
        assert_eq!(format!("{}", r), "127.0.0.1:25565");
    }
}
