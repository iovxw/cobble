use std::fs::{read_to_string, File};
use std::io;
use std::path::Path;
use std::process::exit;
use std::sync::mpsc::{channel, Sender};
use std::{thread, time};

use ozelot::clientbound::*;
use ozelot::{mojang, serverbound, utils, Client};
use rpassword;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Opt {
    /// Your username
    #[structopt(short = "u")]
    username: String,
    /// The server's hostname
    #[structopt(short = "h")]
    host: String,
    /// The server's port
    #[structopt(short = "p", default_value = "25566")]
    port: u16,
    /// Offline mode
    offline: bool,
    /// Path to profile
    #[structopt(short = "c")]
    profile: Option<String>,
}

fn main() {
    let opt = Opt::from_args();

    let mut client = if !opt.offline {
        let username = opt.username; // https://github.com/rust-lang/rust/issues/53488
        let ask_passwd = || {
            let password = rpassword::prompt_password_stdout("Enter password: ").unwrap();
            mojang::Authenticate::new(username, password)
                .perform()
                .unwrap()
        };
        let auth = if let Some(config_path) = opt.profile {
            let config_path = Path::new(&config_path);
            let auth = if config_path.exists() {
                println!("Reading profile...");
                let config = read_to_string(&config_path).expect("failed to read config file");
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
        };
        println!("Authentication successful!, connecting to server...");
        match Client::connect_authenticated(&opt.host, opt.port, &auth) {
            Ok(x) => x,
            Err(e) => {
                println!("Error connecting to {}:{}: {:?}", opt.host, opt.port, e);
                exit(1);
            }
        }
    } else {
        match Client::connect_unauthenticated(&opt.host, opt.port, &opt.username) {
            Ok(x) => x,
            Err(e) => {
                println!(
                    "Error connecting unauthenticated to {}:{}: {:?}",
                    opt.host, opt.port, e
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
        for packet in packets {
            match packet {
                ClientboundPacket::PlayDisconnect(ref p) => {
                    println!("Got disconnect packet, exiting ...");
                    println!("Reason: {}", utils::chat_to_str(p.get_reason()).unwrap());
                    break 'main;
                }
                ClientboundPacket::ChatMessage(ref p) => {
                    let msg = utils::chat_to_str(p.get_chat()).unwrap();
                    println!("{}", msg);
                }
                _ => (),
            }
        }

        if let Ok(msg) = rx.try_recv() {
            let msg = msg.trim_end().to_string();
            let chat = serverbound::ChatMessage::new(msg);
            client.send(chat).unwrap();
        }

        thread::sleep(time::Duration::from_millis(50));
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
    #[serde(default = "always_false")]
    legacy: bool,
    #[serde(default = "always_false")]
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

/// For use with Serde default values
fn always_false() -> bool {
    false
}
