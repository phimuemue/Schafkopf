use primitives::*;
use rules::*;
use itertools::Itertools;

use permutohedron::LexicalPermutation;
use std::fs;
use std::io::Write;
use std::io;

pub struct SSuspicionTransition {
    m_stich : SStich,
    m_susp : SSuspicion,
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
    fn new(susp: &SSuspicion, stich: SStich, rules: &TRules) -> SSuspicionTransition {
        let susp = SSuspicion::new_from_susp(susp, &stich, rules);
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
                    try!(file_output.write_all(b" "));
                }
                try!(file_output.write_all(&format!("{} : ", self.m_stich).as_bytes()));
                if 1<self.m_susp.hand_size() {
                    try!(self.m_susp.print_suspicion(n_maxlevel, n_level, rules, vecstich, ostich_given, &mut file_output));
                } else {
                    try!(file_output.write_all(b""));
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
    m_eplayerindex_first : EPlayerIndex,
    m_ahand : [SHand; 4],
}

impl SSuspicion {

    pub fn suspicion_tranitions(&self) -> &Vec<SSuspicionTransition> {
        &self.m_vecsusptrans
    }

    pub fn new_from_raw(eplayerindex_first: EPlayerIndex, ahand: [SHand; 4]) -> Self {
        SSuspicion {
            m_vecsusptrans: Vec::new(),
            m_eplayerindex_first : eplayerindex_first,
            m_ahand : ahand
        }
    }

    pub fn hands(&self) -> &[SHand; 4] {
        &self.m_ahand
    }

    pub fn count_leaves(&self) -> usize {
        if self.m_vecsusptrans.len()==0 {
            1
        } else {
            self.m_vecsusptrans.iter()
                .fold(0, |n_size_acc, susptrans| n_size_acc + susptrans.m_susp.count_leaves())
        }
    }

    fn new_from_susp(&self, stich: &SStich, rules: &TRules) -> Self {
        //println!("new_from_susp {}", stich);
        //println!("wi: {}", rules.winner_index(stich));
        SSuspicion {
            m_vecsusptrans: Vec::new(),
            m_eplayerindex_first : rules.winner_index(stich),
            m_ahand : create_playerindexmap(|eplayerindex| {
                self.m_ahand[eplayerindex].new_from_hand(stich[eplayerindex])
            })
        }
    }

    fn hand_size(&self) -> usize {
        assert_eq!(self.m_ahand[0].cards().len(), self.m_ahand[1].cards().len());
        assert_eq!(self.m_ahand[0].cards().len(), self.m_ahand[2].cards().len());
        assert_eq!(self.m_ahand[0].cards().len(), self.m_ahand[3].cards().len());
        self.m_ahand[0].cards().len()
    }

    pub fn compute_successors<FuncFilterSuccessors>(&mut self, rules: &TRules, vecstich: &mut Vec<SStich>, func_filter_successors: &FuncFilterSuccessors)
        where FuncFilterSuccessors : Fn(&Vec<SStich> /*vecstich_complete*/, &mut Vec<SStich>/*vecstich_successor*/)
    {
        assert_eq!(self.m_vecsusptrans.len(), 0); // currently, we have no caching
        let mut vecstich_successor : Vec<SStich> = Vec::new();
        push_pop_vecstich(vecstich, SStich::new(self.m_eplayerindex_first), |vecstich| {
            let eplayerindex_first = self.m_eplayerindex_first;
            let player_index = move |i_raw: usize| {(eplayerindex_first + i_raw) % 4};
            macro_rules! traverse_valid_cards {
                ($i_raw : expr, $func: expr) => {
                    // TODO use equivalent card optimization
                    for card in rules.all_allowed_cards(vecstich, &self.m_ahand[player_index($i_raw)]) {
                        vecstich.last_mut().unwrap().zugeben(card);
                        assert!(card==vecstich.last().unwrap()[player_index($i_raw)]);
                        $func;
                        vecstich.last_mut().unwrap().undo_most_recent_card();
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
                let mut susptrans = SSuspicionTransition::new(self, stich.clone(), rules);
                push_pop_vecstich(vecstich, stich, |vecstich| {
                    susptrans.m_susp.compute_successors(rules, vecstich, func_filter_successors);
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
            for eplayerindex in 0..4 {
                try!(file_output.write_all(&format!("{} | ", self.m_ahand[eplayerindex]).as_bytes()));
            }
            try!(file_output.write_all(b", min payouts: "));
            for eplayerindex in 0..4 {
                try!(file_output.write_all(&format!("TODO: payout").as_bytes()));
            }
            try!(file_output.write_all(b""));
            for susptrans in self.m_vecsusptrans.iter() {
                try!(susptrans.print_suspiciontransition(n_maxlevel, n_level+1, rules, vecstich, ostich_given.clone(), &mut file_output));
            }
            Ok(())
        }
    }

    pub fn min_reachable_payout(
        &self,
        rules: &TRules,
        vecstich: &mut Vec<SStich>,
        ostich_given: Option<SStich>,
        eplayerindex: EPlayerIndex
    ) -> isize {
        let vecstich_backup = vecstich.clone();
        assert!(ostich_given.as_ref().map_or(true, |stich| stich.size() < 4));
        assert!(vecstich.iter().all(|stich| stich.size()==4));
        assert_eq!(vecstich.len()+self.hand_size(), 8);
        if 0==self.hand_size() {
            return rules.payout(vecstich)[eplayerindex];
        }
        let n_payout = self.m_vecsusptrans.iter()
            .filter(|susptrans| { // only consider successors compatible with current stich_given so far
                assert_eq!(susptrans.m_susp.hand_size()+1, self.hand_size());
                ostich_given.as_ref().map_or(true, |stich_given| {
                    stich_given.indices_and_cards()
                        .zip(susptrans.m_stich.indices_and_cards())
                        .all(|((i_current_stich, card_current_stich), (i_susp_stich, card_susp_stich))| {
                            assert_eq!(i_current_stich, i_susp_stich);
                            card_current_stich==card_susp_stich
                        })
                })
            })
            .map(|susptrans| {
                assert_eq!(susptrans.m_stich.size(), 4);
                push_pop_vecstich(vecstich, susptrans.m_stich.clone(), |vecstich| {
                    (susptrans, susptrans.m_susp.min_reachable_payout(rules, vecstich, None, eplayerindex))
                })
            })
            .group_by(|&(susptrans, _n_payout)| { // other players may play inconveniently for eplayerindex...
                susptrans.m_stich.indices_and_cards()
                    .take_while(|&(eplayerindex_stich, _card)| eplayerindex_stich != eplayerindex)
                    .map(|(_eplayerindex, card)| card)
                    .collect::<Vec<_>>();
            })
            .map(|(_stich_key_before_eplayerindex, grpsusptransn_before_eplayerindex)| {
                grpsusptransn_before_eplayerindex.into_iter()
                    .group_by(|&(susptrans, _n_payout)| susptrans.m_stich[eplayerindex])
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

pub fn for_each_suspicion<FuncFilter, Func>(
    hand_known: &SHand,
    veccard_unknown : &Vec<SCard>,
    eplayerindex: EPlayerIndex,
    mut func_filter: FuncFilter,
    mut func: Func
)
    where Func: FnMut(SSuspicion),
          FuncFilter: FnMut(&SSuspicion) -> bool
{
    assert_eq!(0, eplayerindex); // TODO: generalize
    let n_cards_total = veccard_unknown.len();
    assert_eq!(n_cards_total%3, 0);
    let n_cards_per_player = n_cards_total / 3;
    let mut veci : Vec<usize> = (0..n_cards_total).map(|i| i/n_cards_per_player).collect();
    let mut callback = |veci : &Vec<usize>| {
        let get_hand = |eplayerindex_hand| {
            SHand::new_from_vec(veci.iter().enumerate()
                .filter(|&(_i, eplayerindex_susp)| *eplayerindex_susp == eplayerindex_hand)
                .map(|(i, _eplayerindex_susp)| veccard_unknown[i.clone()]).collect())
        };
        let susp = SSuspicion::new_from_raw(
            eplayerindex,
            [
                hand_known.clone(),
                get_hand(0),
                get_hand(1),
                get_hand(2),
            ]

        );
        if func_filter(&susp) {
            func(susp);
        }
    };
    callback(&veci);
    while veci[..].next_permutation() {
        callback(&veci);
    }
}
