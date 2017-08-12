extern crate rand;
extern crate ncurses;
#[macro_use]
extern crate itertools;
extern crate permutohedron;
#[macro_use]
extern crate clap;
extern crate arrayvec;
extern crate crossbeam;
#[macro_use]
extern crate error_chain;
extern crate as_num;
#[macro_use]
extern crate plain_enum;
#[macro_use]
extern crate derive_new;
extern crate toml;

#[macro_use]
mod util;
mod primitives;
mod rules;
mod game;
mod player;
mod ai;
mod skui;

use game::*;
use std::sync::mpsc;
use primitives::*;
use rules::TActivelyPlayableRules; // TODO improve trait-object behaviour
use rules::ruleset::*;
use rules::wrappers::*;
use ai::*;
use std::path::Path;
use player::*;
use player::playerhuman::*;
use player::playercomputer::*;
use util::*;

mod errors {
    error_chain!{
        foreign_links {
            Io(::std::io::Error);
            TomlErr(::toml::de::Error);
        }
    }
}

fn main() {
    let clapmatches = clap::App::new("schafkopf")
        .arg(clap::Arg::with_name("rulesetpath")
            .long("ruleset")
            .default_value("ruleset_default.toml")
        )
        .arg(clap::Arg::with_name("numgames")
            .long("numgames")
            .default_value("4")
        )
        .arg(clap::Arg::with_name("ai")
            .long("ai")
            .default_value("cheating")
        )
        .subcommand(clap::SubCommand::with_name("rank-rules")
            .arg(clap::Arg::with_name("hand")
                .long("hand")
                .default_value("")
            )
            .arg(clap::Arg::with_name("pos")
                 .long("position")
                 .default_value("0")
            )
        )
        .get_matches();

    let ai = || {
        match clapmatches.value_of("ai").unwrap().as_ref() {
            "cheating" => Box::new(ai::SAiCheating::new(/*n_rank_rules_samples*/50)) as Box<TAi>,
            "simulating" => 
                Box::new(ai::SAiSimulating::new(
                    /*n_suggest_card_branches*/2,
                    /*n_suggest_card_samples*/10,
                    /*n_rank_rules_samples*/50,
                )) as Box<TAi>,
            _ => {
                println!("Warning: AI not recognized. Defaulting to 'cheating'");
                Box::new(ai::SAiCheating::new(/*n_rank_rules_samples*/50)) as Box<TAi>
            }
        }
    };

    match SRuleSet::from_file(Path::new(clapmatches.value_of("rulesetpath").unwrap())) {
        Ok(ruleset) => {
            if let Some(subcommand_matches)=clapmatches.subcommand_matches("rank-rules") {
                if let Some(str_hand) = subcommand_matches.value_of("hand") {
                    if let Some(hand_fixed) = cardvector::parse_cards(str_hand).map(SHand::new_from_vec) {
                        let epi_rank = value_t!(subcommand_matches.value_of("pos"), EPlayerIndex).unwrap_or(EPlayerIndex::EPI0);
                        println!("Hand: {}", hand_fixed);
                        for rules in allowed_rules(&ruleset.avecrulegroup[epi_rank]).iter() 
                            .filter(|rules| rules.can_be_played(&SFullHand::new(&hand_fixed, ruleset.ekurzlang)))
                        {
                            println!("{}: {}",
                                rules,
                                ai().rank_rules(
                                    &SFullHand::new(&hand_fixed, ruleset.ekurzlang),
                                    EPlayerIndex::EPI0,
                                    epi_rank,
                                    rules.as_rules(),
                                    /*n_stock*/0, // assume no stock in subcommand rank-rules
                                )
                            );
                        }
                    } else {
                        println!("Could not convert \"{}\" to cards.", str_hand);
                    }
                }
                return;
            }

            skui::init_ui();
            let accountbalance = game_loop(
                &EPlayerIndex::map_from_fn(|epi| -> Box<TPlayer> {
                    if EPlayerIndex::EPI0==epi {
                        Box::new(SPlayerHuman{ai : ai()})
                    } else {
                        Box::new(SPlayerComputer{ai: ai()})
                    }
                }),
                /*n_games*/ clapmatches.value_of("numgames").unwrap().parse::<usize>().unwrap_or(4),
                &ruleset,
            );
            skui::end_ui();
            println!("Results: {}", skui::account_balance_string(&accountbalance));
        },
        Err(str_err) => {
            println!("{}", str_err);
        },
    }
}

fn communicate_via_channel<T, Func>(f: Func) -> T
    where Func: FnOnce(mpsc::Sender<T>) -> (),
{
    let (txt, rxt) = mpsc::channel::<T>();
    f(txt.clone());
    rxt.recv().unwrap()
}

fn game_loop(aplayer: &EnumMap<EPlayerIndex, Box<TPlayer>>, n_games: usize, ruleset: &SRuleSet) -> SAccountBalance {
    let mut accountbalance = SAccountBalance::new(EPlayerIndex::map_from_fn(|_epi| 0), 0);
    for i_game in 0..n_games {
        let mut dealcards = SDealCards::new(/*epi_first*/EPlayerIndex::wrapped_from_usize(i_game), ruleset);
        while let Some(epi) = dealcards.which_player_can_do_something() {
            let b_doubling = communicate_via_channel(|txb_doubling| {
                aplayer[epi].ask_for_doubling(
                    dealcards.first_hand_for(epi),
                    txb_doubling
                );
            });
            dealcards.announce_doubling(epi, b_doubling).unwrap();
        }
        let mut gamepreparations = dealcards.finish_dealing(ruleset, accountbalance.get_stock()).unwrap();
        while let Some(epi) = gamepreparations.which_player_can_do_something() {
            skui::logln(&format!("Asking player {} for game", epi));
            let orules = communicate_via_channel(|txorules| {
                aplayer[epi].ask_for_game(
                    &SFullHand::new(&gamepreparations.ahand[epi], ruleset.ekurzlang),
                    &gamepreparations.gameannouncements,
                    &gamepreparations.ruleset.avecrulegroup[epi],
                    gamepreparations.n_stock,
                    None,
                    txorules
                );
            });
            gamepreparations.announce_game(epi, orules.map(|rules| TActivelyPlayableRules::box_clone(rules))).unwrap();
        }
        skui::logln("Asked players if they want to play. Determining rules");
        let stockorpregame = match gamepreparations.finish() {
            VGamePreparationsFinish::DetermineRules(mut determinerules) => {
                while let Some((epi, vecrulegroup_steigered))=determinerules.which_player_can_do_something() {
                    if let Some(rules) = communicate_via_channel(|txorules| {
                        aplayer[epi].ask_for_game(
                            &SFullHand::new(&determinerules.ahand[epi], ruleset.ekurzlang),
                            /*gameannouncements*/&SPlayersInRound::new(determinerules.doublings.first_playerindex()),
                            &vecrulegroup_steigered,
                            determinerules.n_stock,
                            Some(determinerules.currently_offered_prio()),
                            txorules
                        );
                    }) {
                        determinerules.announce_game(epi, TActivelyPlayableRules::box_clone(rules)).unwrap();
                    } else {
                        determinerules.resign(epi).unwrap();
                    }
                }
                VStockOrT::OrT(determinerules.finish().unwrap())
            },
            VGamePreparationsFinish::DirectGame(pregame) => {
                VStockOrT::OrT(pregame)
            },
            VGamePreparationsFinish::Stock(n_stock) => {
                VStockOrT::Stock(n_stock)
            }
        };
        match stockorpregame {
            VStockOrT::OrT(mut pregame) => {
                while let Some(epi_stoss) = pregame.which_player_can_do_something().into_iter()
                    .find(|epi| {
                        communicate_via_channel(|txb_stoss| {
                            aplayer[*epi].ask_for_stoss(
                                *epi,
                                &pregame.doublings,
                                pregame.rules.as_ref(),
                                &pregame.ahand[*epi],
                                &pregame.vecstoss,
                                pregame.n_stock,
                                txb_stoss,
                            );
                        })
                    })
                {
                    pregame.stoss(epi_stoss).unwrap();
                }
                let mut game = pregame.finish();
                while let Some(epi)=game.which_player_can_do_something() {
                    let card = communicate_via_channel(|txcard| {
                        aplayer[epi].ask_for_card(
                            &game,
                            txcard.clone()
                        );
                    });
                    game.zugeben(card, epi).unwrap();
                }
                accountbalance.apply_payout(&game.payout());
            },
            VStockOrT::Stock(n_stock) => {
                accountbalance.apply_payout(&SAccountBalance::new(
                    EPlayerIndex::map_from_fn(|_epi| -n_stock),
                    4*n_stock,
                ));
            }
        }
        skui::print_account_balance(&accountbalance);
    }
    accountbalance
}

#[test]
fn test_game_loop() {
    let mut rng = rand::thread_rng();
    for ruleset in rand::sample(
        &mut rng,
        iproduct!(
            [10, 20].into_iter(), // n_base_price
            [50, 100].into_iter(), // n_solo_price
            [2, 3].into_iter(), // n_lauf_min
            [ // str_allowed_games
                r"
                [rufspiel]
                [solo]
                [wenz]
                lauf-min=2
                ",
                r"
                [solo]
                [farbwenz]
                [wenz]
                [geier]
                ",
                r"
                [solo]
                [wenz]
                [bettel]
                ",
            ].into_iter(),
            [ // str_no_active_game
                r"[ramsch]
                price=20
                ",
                r"[ramsch]
                price=50
                durchmarsch = 75",
                r#"[ramsch]
                price=50
                durchmarsch = "all""#,
                r"[stock]",
                r"[stock]
                price=30",
                r"",
            ].into_iter(),
            [ // str_extras
                r"[steigern]",
                r"[doubling]",
                r#"deck = "kurz""#,
            ].into_iter()
        )
            .map(|(n_base_price, n_solo_price, n_lauf_min, str_allowed_games, str_no_active_game, str_extras)| {
                let str_ruleset = format!(
                    "base-price={}
                    solo-price={}
                    lauf-min={}
                    {}
                    {}
                    {}",
                    n_base_price, n_solo_price, n_lauf_min, str_allowed_games, str_no_active_game, str_extras
                );
                println!("{}", str_ruleset);
                SRuleSet::from_string(&str_ruleset).unwrap()
            }),
            1,
        )
    {
        game_loop(
            &EPlayerIndex::map_from_fn(|epi| -> Box<TPlayer> {
                Box::new(SPlayerComputer{ai: {
                    if epi<EPlayerIndex::EPI2 {
                        Box::new(ai::SAiCheating::new(/*n_rank_rules_samples*/1))
                    } else {
                        Box::new(ai::SAiSimulating::new(/*n_suggest_card_branches*/1, /*n_suggest_card_samples*/1, /*n_samples_per_rules*/1))
                    }
                }})
            }),
            /*n_games*/4,
            &ruleset,
        );
    }
}
