use anyhow::Result;
use clap::{arg, ArgAction, ArgMatches, Command};

pub fn subcmd<'help>() -> Command<'help> {
    Command::new("cat")
        .about("output messages from the usb device")
        .arg(arg!( -p --pretty "set to pretty print values").action(ArgAction::SetTrue))
}

pub async fn cmd(data: &mut super::CmdData, m: &ArgMatches) -> Result<()> {
    let pretty = *m.get_one::<bool>("pretty").unwrap();
    loop {
        let msg = data.device.read().await;
        if pretty {
            println!("{}", serde_json::to_string_pretty(&msg).unwrap())
        } else {
            println!("{}", serde_json::to_string(&msg).unwrap())
        }
    }
}
