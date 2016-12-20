use primitives::*;
use rules::*;
use itertools::Itertools;

use std::fs;
use std::io::Write;
use std::io;

pub struct SSuspicionTransition {
    m_stich : SStich,
    m_susp : SSuspicion,
}

pub fn assert_ahand_same_size(ahand: &SPlayerIndexMap<SHand>) {
    let n_len_hand = ahand[0].cards().len();
    assert!(ahand.iter().all(|hand| hand.cards().len()==n_len_hand));
}


pub fn push_pop_vecstich<Func, R>(vecstich: &mut Vec<SStich>, stich: SStich, func: Func) -> R
    where Func: FnOnce(&mut Vec<SStich>) -> R
{
    let n_stich = vecstich.len();
    assert!(vecstich.iter().all(|stich| stich.size()==4));
    vecstich.push(stich);
    let r = func(vecstich);
    vecstich.pop().expect("vecstich unexpectedly empty");
    assert!(vecstich.iter().all(|stich| stich.size()==4));
    assert_eq!(n_stich, vecstich.len());
    r
}

impl SSuspicionTransition {
    fn new(susp: &SSuspicion, stich: SStich) -> SSuspicionTransition {
        let susp = SSuspicion::new_from_susp(susp, &stich);
        SSuspicionTransition {
            m_stich : stich,
            m_susp : susp
        }
    }

    pub fn stich(&self) -> &SStich {
        &self.m_stich
    }

    pub fn suspicion(&self) -> &SSuspicion {
        &self.m_susp
    }

    fn print_suspiciontransition(
        &self,
        n_maxlevel: usize,
        n_level: usize,
        rules: &TRules,
        vecstich: &mut Vec<SStich>,
        ostich_given: Option<SStich>,
        mut file_output: &mut fs::File,
    ) -> io::Result<()> {
        if n_level<=n_maxlevel {
            push_pop_vecstich(vecstich, self.m_stich.clone(), |vecstich| {
                assert_eq!(vecstich.len()+self.m_susp.hand_size(), 8);
                for _ in 0..n_level+1 {
                    file_output.write_all(b" ")?;
                }
                file_output.write_all(&format!("{} : ", self.m_stich).as_bytes())?;
                if 1<self.m_susp.hand_size() {
                    self.m_susp.print_suspicion(n_maxlevel, n_level, rules, vecstich, ostich_given, &mut file_output)?;
                } else {
                    file_output.write_all(b"\n")?;
                }
                Ok(())
            })
        } else {
            Ok(())
        }
    }
}

pub struct SSuspicion {
    m_vecsusptrans : Vec<SSuspicionTransition>,
    m_ahand : SPlayerIndexMap<SHand>,
}

impl SSuspicion {

    pub fn suspicion_transitions(&self) -> &Vec<SSuspicionTransition> {
        &self.m_vecsusptrans
    }

    pub fn new<FuncFilterSuccessors>(
        eplayerindex_first: EPlayerIndex,
        ahand: SPlayerIndexMap<SHand>,
        rules: &TRules,
        vecstich: &mut Vec<SStich>,
        func_filter_successors: FuncFilterSuccessors,
    ) -> Self 
        where FuncFilterSuccessors : Fn(&Vec<SStich> /*vecstich_complete*/, &mut Vec<SStich>/*vecstich_successor*/)
    {
        let mut susp = SSuspicion {
            m_vecsusptrans: Vec::new(),
            m_ahand : ahand
        };
        susp.compute_successors(eplayerindex_first, rules, vecstich, &func_filter_successors);
        susp
    }

    pub fn count_leaves(&self) -> usize {
        if self.m_vecsusptrans.len()==0 {
            1
        } else {
            self.m_vecsusptrans.iter()
                .map(|susptrans| susptrans.m_susp.count_leaves())
                .sum()
        }
    }

    fn new_from_susp(&self, stich: &SStich) -> Self {
        SSuspicion {
            m_vecsusptrans: Vec::new(),
            m_ahand : create_playerindexmap(|eplayerindex| {
                self.m_ahand[eplayerindex].new_from_hand(stich[eplayerindex])
            })
        }
    }

    fn hand_size(&self) -> usize {
        assert_ahand_same_size(&self.m_ahand);
        self.m_ahand[0].cards().len()
    }

    fn compute_successors<FuncFilterSuccessors>(&mut self, eplayerindex_first: EPlayerIndex, rules: &TRules, vecstich: &mut Vec<SStich>, func_filter_successors: &FuncFilterSuccessors)
        where FuncFilterSuccessors : Fn(&Vec<SStich> /*vecstich_complete*/, &mut Vec<SStich>/*vecstich_successor*/)
    {
        assert_eq!(self.m_vecsusptrans.len(), 0); // currently, we have no caching
        let mut vecstich_successor : Vec<SStich> = Vec::new();
        push_pop_vecstich(vecstich, SStich::new(eplayerindex_first), |vecstich| {
            let offset_to_playerindex = move |i_offset: usize| {eplayerindex_wrapping_add(eplayerindex_first, i_offset)};
            macro_rules! traverse_valid_cards {
                ($i_offset : expr, $func: expr) => {
                    // TODO use equivalent card optimization
                    for card in rules.all_allowed_cards(vecstich, &self.m_ahand[offset_to_playerindex($i_offset)]) {
                        vecstich.last_mut().unwrap().push(card);
                        assert!(card==vecstich.last().unwrap()[offset_to_playerindex($i_offset)]);
                        $func;
                        vecstich.last_mut().unwrap().undo_most_recent();
                    }
                };
            };
            traverse_valid_cards!(0, { // TODO: more efficient to explicitly handle first card?
                traverse_valid_cards!(1, {
                    traverse_valid_cards!(2, {
                        traverse_valid_cards!(3, {
                            vecstich_successor.push(vecstich.last().unwrap().clone());
                        } );
                    } );
                } );
            } );
        });
        if !vecstich_successor.is_empty() {
            func_filter_successors(vecstich, &mut vecstich_successor);
            assert!(!vecstich_successor.is_empty());
        }
        self.m_vecsusptrans = vecstich_successor.into_iter()
            .map(|stich| {
                let mut susptrans = SSuspicionTransition::new(self, stich.clone());
                let eplayerindex_first_susp = rules.winner_index(&stich);
                push_pop_vecstich(vecstich, stich, |vecstich| {
                    susptrans.m_susp.compute_successors(eplayerindex_first_susp, rules, vecstich, func_filter_successors);
                });
                susptrans
            })
            .collect();
    }

    pub fn print_suspicion(
        &self,
        n_maxlevel: usize,
        n_level: usize,
        rules: &TRules,
        vecstich: &mut Vec<SStich>,
        ostich_given: Option<SStich>,
        mut file_output: &mut fs::File,
    ) -> io::Result<()> {
        if n_maxlevel < n_level {
            Ok(())
        } else {
            for eplayerindex in eplayerindex_values() {
                file_output.write_all(&format!("{} | ", self.m_ahand[eplayerindex]).as_bytes())?;
            }
            file_output.write_all(b", min payouts: ")?;
            for _eplayerindex in eplayerindex_values() {
                file_output.write_all(&format!("TODO: payout").as_bytes())?;
            }
            file_output.write_all(b"\n")?;
            for susptrans in self.m_vecsusptrans.iter() {
                susptrans.print_suspiciontransition(n_maxlevel, n_level+1, rules, vecstich, ostich_given.clone(), &mut file_output)?;
            }
            Ok(())
        }
    }

    pub fn min_reachable_payout(
        &self,
        rules: &TRules,
        vecstich: &mut Vec<SStich>,
        ostich_given: Option<SStich>,
        eplayerindex: EPlayerIndex,
        n_stoss: usize,
        n_doubling: usize,
        n_stock: isize,
    ) -> isize {
        let vecstich_backup = vecstich.clone();
        assert!(ostich_given.as_ref().map_or(true, |stich| stich.size() < 4));
        assert!(vecstich.iter().all(|stich| stich.size()==4));
        assert_eq!(vecstich.len()+self.hand_size(), 8);
        if 0==self.hand_size() {
            return rules.payout(&SGameFinishedStiche::new(vecstich), n_stoss, n_doubling, n_stock).get_player(eplayerindex);
        }
        let n_payout = self.m_vecsusptrans.iter()
            .filter(|susptrans| { // only consider successors compatible with current stich_given so far
                assert_eq!(susptrans.m_susp.hand_size()+1, self.hand_size());
                ostich_given.as_ref().map_or(true, |stich_given| {
                    stich_given.iter()
                        .zip(susptrans.m_stich.iter())
                        .all(|((i_current_stich, card_current_stich), (i_susp_stich, card_susp_stich))| {
                            assert_eq!(i_current_stich, i_susp_stich);
                            card_current_stich==card_susp_stich
                        })
                })
            })
            .map(|susptrans| {
                assert_eq!(susptrans.m_stich.size(), 4);
                push_pop_vecstich(vecstich, susptrans.m_stich.clone(), |vecstich| {
                    (susptrans, susptrans.m_susp.min_reachable_payout(rules, vecstich, None, eplayerindex, n_stoss, n_doubling, n_stock))
                })
            })
            .group_by(|&(susptrans, _n_payout)| { // other players may play inconveniently for eplayerindex...
                susptrans.m_stich.iter()
                    .take_while(|&(eplayerindex_stich, _card)| eplayerindex_stich != eplayerindex)
                    .map(|(_eplayerindex, card)| card)
                    .collect::<Vec<_>>();
            })
            .into_iter()
            .map(|(_stich_key_before_eplayerindex, grpsusptransn_before_eplayerindex)| {
                grpsusptransn_before_eplayerindex.into_iter()
                    .group_by(|&(susptrans, _n_payout)| susptrans.m_stich[eplayerindex])
                    .into_iter()
                    .map(|(_stich_key_eplayerindex, grpsusptransn_eplayerindex)| {
                        // in this group, we need the worst case if other players play badly
                        grpsusptransn_eplayerindex.into_iter().min_by_key(|&(_susptrans, n_payout)| n_payout).unwrap()
                    })
                    .max_by_key(|&(_susptrans, n_payout)| n_payout)
                    .unwrap()
            })
            .min_by_key(|&(_susptrans, n_payout)| n_payout)
            .unwrap()
            .1;
        assert!(vecstich_backup.iter().zip(vecstich.iter()).all(|(s1,s2)|s1.size()==s2.size()));
        n_payout
    }

}

