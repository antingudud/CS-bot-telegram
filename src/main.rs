use std::error::Error;

use teloxide::prelude::*;
use teloxide::types::InputFile;
use teloxide::utils::command::parse_command;

use tokio::join;

use axum::{
    routing::post,
    Router, Json,
    http::StatusCode
};

use serde::{Deserialize, Serialize};

use url::Url;

#[tokio::main]
async fn main() {

    pretty_env_logger::init();
    let bot = Bot::from_env();
    println!("Telegram bot is running");

    let _ = join!(
        run_server(bot.clone()),
        run_telegram(bot.clone())
    );
}

async fn send_request(msg: Msg) -> Result<Option<u8>, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let json = serde_json::to_string(&msg)?;
    let res = client.post("http://127.0.0.1:3030/post-message")
        .body(json.clone())
        .header("Content-Type", "application/json")
        .send()
        .await?
        .json::<ResponseMsg>()
        .await?;
    println!("[LOG] Request JSON: {}", &json);

    //println!("Request response:\n{}\n{}", res.status(), res.text().await?);
    if res.status.eq("fail") {
        if res.code == 2 {
            println!("[ERROR] at send_request: STATUS OF {} NO FORUM: {}", res.status, res.message);
            return Ok(Some(2));
        }
        if res.code == 1 {
            println!("[INFO] send_request: STATUS OF {} {}", res.status, res.message);
        }
        return Ok(Some(1));
    } 
    Ok(None)
}

// TODO: there should be only one send_request
async fn send_init_request(params: PayloadInit) -> Result<Option<u32>, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let json = serde_json::to_string(&params)?;
    let res = client.post("http://127.0.0.1:3030/init")
        .body(json.clone())
        .header("Content-Type", "application/json")
        .send()
        .await?
        .json::<ResponseForum>()
        .await?;
    //println!("{}", &json);

    println!("Request code: {}", res.code);
    if res.status.eq("fail"){
        if res.code == 1 {return Ok(Some(1))};
        if res.message.eq("forum exists") {return Ok(Some(2))};
        return Ok(Some(1));
    }

    //println!("Request response:\n{}\n{}", res.status(), res.text().await?);
    Ok(None)
}

async fn send_close_request(params: PayloadClose) -> Result<Option<u8>, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let json = serde_json::to_string(&params)?;
    let res = client.post("http://127.0.0.1:3030/close")
        .body(json)
        .header("Content-Type", "application/json")
        .send()
        .await?
        .json::<ResponseMsg>()
        .await?;

    if res.status.eq("fail") {
        if res.code == 1 {return Ok(Some(1))};
        if res.message.eq("forum not found") {return Ok(Some(2))};
        return Ok(Some(1));
    }

    //println!("Request response:\n{}\n{}", res.status(), res.text().await?);
    Ok(None)
}

fn make_forum_payload(chat_id: ChatId, title: String) -> PayloadInit {
    let params: PayloadInit = PayloadInit {
        id: chat_id.0,
        title
    };
    params
    //send_init_request(params).await?;
    //bot.send_message(chat_id, "Terima kasih, pesan anda berikutnya akan dikirimkan ke tim Customer Service kami.".to_string()).await?;
    //Ok(())
}

async fn bukatiket_handler(bot: &Bot, chid: ChatId, args: Vec<&str>) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        bot.send_message(chid, "Mohon maaf, tolong ketik judul di setelah '/bukatiket'.".to_string()).await?;
        return Ok(());
    }

    let pl: PayloadInit = make_forum_payload(chid, args.join(" "));

    let i: u8 = match send_init_request(pl).await {
        Ok(x) => {
            match x {
                Some(v) => {
                    if v == 1 {1}
                    else {1}
                },
                None => 0
            }
        },
        Err(why) => {
            println!("[ERROR] at telegram_handler in bukatiket: {:?}", why);
            1
        }
    };

    if i == 2 {
        bot.send_message(chid, "Mohon maaf, saat ini anda sedang berada dalam tiket aktif. Mohon untuk menutup tiket ini jika ingin membuka tiket baru.".to_string()).await?;
    } else if i == 1 {
        bot.send_message(chid, "Mohon maaf, terjadi kesalahan ketika membuka tiket. Mohon coba lagi di lain waktu.".to_string()).await?;
    } else if i == 0{
        bot.send_message(chid, "Terima kasih, pesan anda berikutnya akan dikirimkan ke tim Customer Service kami.".to_string()).await?;
    }

    Ok(())
}

async fn tutuptitket_handler(bot: &Bot, chid: ChatId) -> Result<(), Box<dyn Error>> {
    let pl = PayloadClose {
        id: chid.0.clone()
    };

    let i: u8 = match send_close_request(pl).await? {
        Some(x) => x,
        None => 0
    };

    if i == 1 {
        bot.send_message(chid, "Mohon maaf, terjadi kesalahan ketika menutup tiket. Mohon coba lagi di lain waktu.".to_string()).await?;
    } else if i == 2 {
        bot.send_message(chid, "Mohon maaf, tiket yang dimaksud tidak ada. Mohon coba lagi di lain waktu.").await?;
    } else if i == 0 {
        bot.send_message(chid, "Tiket sudah ditutup, terima kasih sudah menggunakan layanan kami.".to_string()).await?;
    }

    Ok(())
}

async fn telegram_handler(msg: Message, bot: Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    //println!("Chat Id: {}", msg.chat.id);
    //println!("TextL {}", msg.text().unwrap());
    let chat_id: i64 = msg.chat.id.0;
    if chat_id < 0 {
        bot.send_message(msg.chat.id, "Bot ini hanya bisa dijalankan di dalam pesan langsung atau DM.").await?;
        return Ok(());
    }
    let mut caption: Option<String> = None;
    let attachment: Vec<(String, String)> = match get_file_id(&msg) {
        Some((id, capt)) => {
            caption = capt;
            let file: String = bot.get_file(&id).await.unwrap().path;
            let filename: Vec<String> = file.split("/").map(|s| s.to_string()).collect();
            println!("[LOG] File path: {:?}", &file);
            let path: String = format!("https://api.telegram.org/file/bot{}/{}", &bot.token(), &file);
            vec!((filename.last().unwrap().to_string(), path))
        },
        None => Vec::new()
    };

    let text: String = match msg.text() {
        Some(x) => {
            x.to_string()
        },
        None => {
            match caption {
                Some(x) => x,
                None => String::new()
            }
        }
    };
    let (command, args) = parse_command(&text, "Telecord").unwrap_or(("", vec!("")));

    if command.eq("bukatiket") {
        let _ = bukatiket_handler(&bot, msg.chat.id.clone(), args).await;
        return Ok(());
    } else if command.eq("tutuptiket"){
        let _ = tutuptitket_handler(&bot, msg.chat.id.clone()).await;
        return Ok(());
    } else if command.eq("start") {
        bot.send_message(msg.chat.id, "Halo, selamat datang di Customer Service Icommits! Ada yang bisa saya bantu?\n\nUntuk membuat tiket baru, kirim /bukatiket <deskripsi singkat permasalahan>\nJika sudah selesai, anda bisa kirim /tutuptiket untuk menutupnya.".to_string()).await?;
        return Ok(());
    }
    
    let author: String = match msg.from() {
        Some(x) => x.full_name(),
        None => String::from("Unknown User")
    };
    let smesg: Msg = Msg {
        tele_id: chat_id,
        author,
        text,
        attachment
    };
    println!("INFO: {:?}", smesg);

    let sus = match send_request(smesg).await {
        Ok(x) => x,
        Err(why) => {
            println!("[ERROR] Error at sending request in telegram_handler: {:?}", why);
            Some(1)
        }
    };

    let i: u8 = match sus {
        Some(x) => x,
        None => 0
    };

    if i == 2 {
        bot.send_message(msg.chat.id, "Mohon untuk buka tiket baru.".to_string()).await?;
    } else if i == 1 {
        bot.send_message(msg.chat.id, "Mohon maaf, telah terjadi kesalahan. Mohon coba lagi beberapa saat nanti.").await?;
    }

    Ok(())
}

// Option<(file_id: String, Option<caption: String>>
fn get_file_id(m: &Message) -> Option<(String, Option<String>)> {
    #[allow(unused_assignments)] // its literally checked if its "empty" or not just below. TODO:
                                 // use Option
    let mut id = "empty".to_string();
    #[allow(unused_mut)] // whats just below this then??
    let mut caption: Option<String>;

    if m.photo().is_some() {
        caption = match m.caption() {
            Some(x) => Some(x.to_string()),
            None => None
        };
        id = m.photo().unwrap().last().unwrap().file.id.clone();
    } else if m.audio().is_some() {
        caption = match m.caption() {
            Some(x) => Some(x.to_string()),
            None => None
        };
        id = m.audio().unwrap().file.id.clone();
    } else if m.document().is_some() {
        caption = match m.caption() {
            Some(x) => Some(x.to_string()),
            None => None
        };
        id = m.document().unwrap().file.id.clone();
    } else if m.animation().is_some() {
        caption = match m.caption() {
            Some(x) => Some(x.to_string()),
            None => None
        };
        id = m.animation().unwrap().file.id.clone();
    } else if m.sticker().is_some() {
        caption = match m.caption() {
            Some(x) => Some(x.to_string()),
            None => None
        };
        id = m.sticker().unwrap().file.id.clone();
    } else if m.video().is_some() {
        caption = match m.caption() {
            Some(x) => Some(x.to_string()),
            None => None
        };
        id = m.video().unwrap().file.id.clone();
    } else if m.voice().is_some() {
        caption = match m.caption() {
            Some(x) => Some(x.to_string()),
            None => None
        };
        id = m.voice().unwrap().file.id.clone();
    } else {
        return None;
    }

    if id.eq("empty") != true {
        println!("Media id : {:?}", id);
    } else {
        return None;
    }
    Some((id, caption))
}

async fn run_telegram(bot: Bot) -> Result<(), Box<dyn Error>>{
    let handler = dptree::entry().branch(Update::filter_message().endpoint(telegram_handler));
    let mut _dispatcher = Dispatcher::builder(bot.clone(), handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

async fn run_server(bot: Bot) {
    let app = Router::new()
        .route("/post-message", post({
            move |payload| message_controller(payload, bot.clone())
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3031").await.unwrap();
    println!("Server is starting");
    axum::serve(listener, app).await.unwrap();
}

async fn message_controller(Json(payload): Json<Msg>, bot: Bot) -> (StatusCode, Json<String>) {

    let message = format!("{}: {}", payload.author, payload.text);
    let response_message: String = format!("{}. You sent: {}", payload.author, payload.text);
    let ch_id: i64 = payload.tele_id;

    if let Err(why) = send_message(&bot, ch_id, message).await {
        let err_msg = format!("Error at sending message: {:?}", why);
        return (StatusCode::BAD_REQUEST, Json(err_msg))
    };

    let mut err_count: u32 = 0;
    for (_, url) in payload.attachment.iter() {
        if let Err(why) = send_file_from_url(&bot, ch_id, url.to_string()).await {
            println!("Error when sending file: {:?}\n", why);
            err_count += 1;
        }
    };
    if err_count > 0 {
        let err_msg = format!("One or more files sent by {} failed to be sent", payload.author);
        if let Err(why) = send_message(&bot, ch_id, err_msg).await {
            println!("[ERROR] message_controller: Failed to send error message in err_count iterator\n {:?}", why);
        };
    }

    (StatusCode::OK, Json(response_message))
}

async fn send_message(bot: &Bot, ch_id: i64, text: String) -> Result<(), Box<dyn Error>> {
    bot.send_message(ChatId(ch_id), text).await?;
    Ok(())
}

async fn send_file_from_url(bot: &Bot, ch_id: i64, url: String) -> Result<(), Box<dyn Error>> {
    let file: InputFile = inputfile_from_url(url)?;
    let _d = Bot::send_document(&bot, ChatId(ch_id), file).await?;

    Ok(())
}

fn inputfile_from_url(url: String) -> Result<InputFile, Box<dyn Error>> {
    let file: InputFile = InputFile::url(Url::parse(&url)?);

    Ok(file)
}

#[derive(Debug,Serialize,Deserialize)]
struct Msg {
    tele_id: i64,
    author: String, // fullname
    text: String,
    attachment: Vec<(String, String)> // (filename, url)
}

#[derive(Serialize)]
struct PayloadInit {
    id: i64,
    title: String
}

#[derive(Serialize)]
struct PayloadClose {
    id: i64
}

#[derive(Deserialize, Debug)]
struct ResponseForum {
    status: String,
    code: u8, // 0 is normal, 1 is system error, 2 is logic error
    message: String,
    id: u64 // forum post id | 0 is null
}

#[derive(Deserialize, Debug)]
struct ResponseMsg {
    status: String,
    code: u32,
    message: String
}
