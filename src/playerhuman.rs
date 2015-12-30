use stich::*;
use card::*;
use hand::*;
use player::*;
use gamestate::*;
use rules::*;
use rulesrufspiel::*;

use std::sync::mpsc;
use std::io::{self, Read};
use std::fmt::Display;

pub struct CPlayerHuman;

fn ask_for_alternative<T>(vect: &Vec<T>) -> T 
    where T : Display + Clone
{
    assert!(0<vect.len());
    println!("Please choose:");
    loop {
        for (i_t, t) in vect.iter().enumerate() {
            println!("{} ({})", t, i_t);
        }
        let mut str_index = String::new();
        if let Err(e) = (io::stdin().read_line(&mut str_index)) {
            return vect[0].clone(); // TODO: make return type optional?
        }
        match str_index.trim().parse::<usize>() {
            Ok(i) if i < vect.len() => {
                println!("Chosen {}", i);
                return vect[i].clone();
            }
            Ok(_) => {
                println!("Error. Number not within suggested bounds.");
            }
            _ => {
                println!("Error. Input not a number");
            }
        }
    }
}

impl CPlayer for CPlayerHuman {
    fn take_control(&mut self, gamestate: &SGameState, txcard: mpsc::Sender<CCard>) {
        let eplayerindex = gamestate.which_player_can_do_something().unwrap();
        println!("Human player has: {}", gamestate.m_ahand[eplayerindex]);
        let veccard_allowed = gamestate.m_rules.all_allowed_cards(
            &gamestate.m_vecstich,
            &gamestate.m_ahand[eplayerindex]
        );
        println!(
            "Please choose a card (0 to {})",
            veccard_allowed.len()-1,
        );
        txcard.send(ask_for_alternative(&veccard_allowed));
        println!("Sent");
    }

    fn ask_for_game(&self, eplayerindex: EPlayerIndex, _: &CHand) -> Option<Box<TRules>> {
        None // TODO: implement this properly
    }
}
