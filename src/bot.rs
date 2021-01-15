use super::{ChatMessage, Result};
use std::sync::{mpsc::{Receiver, Sender}, Arc, Mutex};
use std::time::Duration;
use std::thread;
use smol::Timer;
use twitchchat::{
    messages::Commands,
    runner::{AsyncRunner, Status},
    UserConfig,
};

pub struct Bot;

async fn sleep(dur: Duration) {
    Timer::new(dur).await;
}


impl Bot {
    pub async fn run(
        &self,
        user_config: &UserConfig,
        channels: &Vec<String>,
        messages_for_chat: Receiver<String>,
        send_incomming_chat_message: Sender<ChatMessage>,
    ) -> Result<()> {
        let connector = twitchchat::connector::smol::Connector::twitch()?;

        let mut tries = 0;
        let messages_for_chat = Arc::new(Mutex::new(messages_for_chat));

        loop {
            if let Ok(mut runner) = AsyncRunner::connect(connector.clone(), user_config).await {
                tries = 0;
                for channel in channels {
                    runner.join(channel).await?;
                }

                if let Ok(_) = self.main_loop(
                    &mut runner,
                    messages_for_chat.clone(),
                    send_incomming_chat_message.clone(),
                    channels.to_vec(),
                ).await {
                        return Ok(());
                }
            } else {
                tries += 1;
                sleep(Duration::from_secs(tries)).await;
                if tries > 3 {
                    panic!("could not connect. retried too often!");
                }
            }
        }
    }

    async fn main_loop<'a>(
        &self,
        runner: &mut AsyncRunner,
        messages_for_chat: Arc<Mutex<Receiver<String>>>,
        send_incomming_chat_message: Sender<ChatMessage>,
        channels: Vec<String>,
    ) -> Result<()> {
        let mut writer = runner.writer();
        let _quit = runner.quit_handle();
        thread::spawn(move || -> Result<()> {
            let messages_for_chat = messages_for_chat.lock().unwrap();
            loop {
                if let Ok(message) = messages_for_chat.recv() {
                    for channel in channels.iter() {
                        let message = twitchchat::commands::privmsg(channel, &message);
                        smol::block_on(writer.encode(message))?;
                    }
                }
            }
        });

        loop {
            match runner.next_message().await? {
                Status::Message(Commands::Privmsg(raw_message)) => {
                    send_incomming_chat_message.send(raw_message.into())?;
                }
                Status::Quit | Status::Eof => break,
                Status::Message(_) => {
                    continue;
                }
            }
        }
        Ok(())
    }
}
