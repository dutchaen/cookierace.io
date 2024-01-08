use std::thread;
use std::time::{Instant, Duration};

use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};

use actix_files::{Files, NamedFile};
use actix_web::{get, HttpServer, App};

use simple_websockets::{
    Event,
    Message,
    Responder
};

type WebSocketClientId = u64;

#[derive(Debug)]
struct Game {
    players: Vec<Player>,
    stop_time: Instant,
}

#[derive(Debug)]
struct Player {
    id: WebSocketClientId,
    click_count: u64
}


#[derive(Debug, Serialize, Deserialize)]
struct CookieEvent {
    event: String,
}


#[get("/")]
async fn main_page() -> Result<actix_files::NamedFile, std::io::Error> {
    return NamedFile::open("index.html");
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    
    thread::spawn(|| start_websocket_server(8081));
    println!("WebSocket server thread has started!");


    println!("HTTP Server is starting now...");
    HttpServer::new(|| {
        App::new()
            .service(main_page)
            .service(Files::new("/static", "static").prefer_utf8(true))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await

}


fn start_websocket_server(port: u16) {
    // listen for WebSockets on port 8080:
    let event_hub = simple_websockets::launch(port)
        .expect("websocket failed to listen on port");
    // map between client ids and the client's `Responder`:

    let connected_event = serde_json::to_string(
        &CookieEvent {
            event: "connected".into()
        }
    ).unwrap();

    let game_won_event = serde_json::to_string(
        &CookieEvent {
            event: "you_win".into()
        }
    ).unwrap();

    let game_lost_event = serde_json::to_string(
        &CookieEvent {
            event: "you_lost".into()
        }
    ).unwrap();

    let mut games = vec![];
    let mut queue: HashSet<WebSocketClientId> = HashSet::new();
    let mut clients: HashMap<u64, Responder> = HashMap::new();

    loop {
        match event_hub.poll_event() {
            Event::Connect(client_id, responder) => {
                println!("A client connected with id #{}", client_id);
                // add their Responder to our `clients` map:
                clients.insert(client_id, responder);
            },
            Event::Disconnect(client_id) => {
                println!("Client #{} disconnected.", client_id);
                // remove the disconnected client from the clients map:
                clients.remove(&client_id);
            },
            Event::Message(client_id, message) => {
                println!("Received a message from client #{}: {:?}", client_id, message);
                // retrieve this client's `Responder`:
                //let responder = clients.get(&client_id).unwrap();

                match message {
                    Message::Text(text) => {
                        let cookiez = match serde_json::from_str::<CookieEvent>(&text) {
                            Ok(event) => event,
                            _ => continue,
                        };
                        
                        match cookiez.event.as_str() {
                            "queue" => {
                                //queue.insert(client_id);
                                if queue.is_empty() {
                                    queue.insert(client_id);
                                    continue;
                                }
                                
                                let other_client = match queue.iter().nth(0) {
                                    Some(id) => *id,
                                    _ => {
                                        queue.insert(client_id);
                                        continue;
                                    },
                                };

                                if other_client == client_id {
                                    continue;
                                }

                                queue.remove(&client_id);
                                queue.remove(&other_client);

                                let me = clients.get(&client_id).unwrap();
                                let other = clients.get(&other_client).unwrap();

                                let end = Instant::now() + Duration::from_secs(30);

                                games.push(Game {
                                    players: vec![
                                        Player {
                                            id: client_id,
                                            click_count: 0,
                                        },
                                        Player {
                                            id: other_client,
                                            click_count: 0,
                                        }
                                    ],
                                    stop_time: end,
                                });

                                me.send(Message::Text(connected_event.clone()));
                                other.send(Message::Text(connected_event.clone()));
                            },
                            "click" => {
                                let (index, game) = match games.iter_mut()
                                    .enumerate()
                                    .find(|(_, x)| x.players.iter()
                                        .find(|y| y.id == client_id)
                                        .is_some()
                                    ) {
                                        Some(game) => game,
                                        _ => continue,
                                    };

                                if game.stop_time < Instant::now() {
                                    // if the game has ended
                                    let mut players = game.players.iter()
                                        .collect::<Vec<_>>();

                                    // ascend to descend score
                                    players.sort_by(|x, y| y.click_count.cmp(&x.click_count));

                                    for (i, player) in players.iter().enumerate() {
                                        let socket = clients.get(&player.id).unwrap();
                                        if i == 0 {
                                            socket.send(Message::Text(game_won_event.to_owned()));
                                        }
                                        else {
                                            socket.send(Message::Text(game_lost_event.to_owned()));
                                        }
                                    }
                                    games.remove(index);
                                    continue;

                                }
                                else {
                                    game.players.iter_mut()
                                        .find(|x| x.id == client_id)
                                        .and_then(|x| Some(x.click_count += 1));
                                }
                            } 
                            _ => {},
                        }
                    },
                    _ => {}
                }  
            },
        }
    }
}