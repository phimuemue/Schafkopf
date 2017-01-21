use primitives::*;
use rules::*;
use rules::trumpfdecider::*;
use rules::payoutdecider::SStossDoublingPayoutDecider;
use rules::card_points::*;
use std::fmt;
use std::cmp::Ordering;
use itertools::Itertools;

pub struct SRulesRamsch {
    pub m_n_price : isize,
}

impl fmt::Display for SRulesRamsch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Ramsch")
    }
}

pub type STrumpfDeciderRamsch = STrumpfDeciderSchlag<
    SSchlagDesignatorOber, STrumpfDeciderSchlag<
    SSchlagDesignatorUnter, STrumpfDeciderFarbe<
    SFarbeDesignatorHerz>>>;

impl TRules for SRulesRamsch {
    impl_rules_trumpf!(STrumpfDeciderRamsch);

    fn stoss_allowed(&self, _eplayerindex: EPlayerIndex, vecstoss: &[SStoss], hand: &SHand) -> bool {
        assert!(vecstoss.is_empty());
        assert_eq!(hand.cards().len(), 8);
        false
    }

    fn playerindex(&self) -> Option<EPlayerIndex> {
        None
    }

    fn payout(&self, gamefinishedstiche: &SGameFinishedStiche, n_stoss: usize, n_doubling: usize, _n_stock: isize) -> SAccountBalance {
        // TODO rules Durchmarsch
        let an_points = gamefinishedstiche.get().iter()
            .fold(
                EPlayerIndex::map_from_fn(|_eplayerindex| 0),
                |mut an_points_accu, stich| {
                    an_points_accu[self.winner_index(stich)] += points_stich(stich);
                    an_points_accu
                }
            );
        let n_points_max = an_points.iter().max().unwrap();
        let veceplayerindex_most_points = EPlayerIndex::values()
            .filter(|eplayerindex| n_points_max==&an_points[*eplayerindex])
            .collect::<Vec<_>>();
        let eplayerindex_loser : EPlayerIndex = {
            if 1==veceplayerindex_most_points.len() {
                veceplayerindex_most_points[0]
            } else {
                veceplayerindex_most_points.into_iter()
                    .map(|eplayerindex| {(
                        eplayerindex,
                        gamefinishedstiche.get().iter()
                            .map(|stich| stich[eplayerindex])
                            .filter(|card| self.trumpforfarbe(*card).is_trumpf())
                            // TODO rust: use max_by
                            .fold1(|card_fst, card_snd| {
                                assert!(self.trumpforfarbe(card_fst).is_trumpf());
                                assert!(self.trumpforfarbe(card_snd).is_trumpf());
                                if Ordering::Less==self.compare_trumpf(card_fst, card_snd) {
                                    card_snd
                                } else {
                                    card_fst
                                }
                            })
                    )})
                    .fold1(|paireplayerindexocard_fst, paireplayerindexocard_snd| {
                        match (paireplayerindexocard_fst.1, paireplayerindexocard_snd.1) {
                            (Some(card_trumpf_fst), Some(card_trumpf_snd)) => {
                                if Ordering::Less==self.compare_trumpf(card_trumpf_fst, card_trumpf_snd) {
                                    paireplayerindexocard_snd
                                } else {
                                    paireplayerindexocard_fst
                                }
                            },
                            (Some(_), None) => paireplayerindexocard_fst,
                            (None, Some(_)) => paireplayerindexocard_snd,
                            // If two ore more players have the maximum number of points,
                            // at least one of them must have had at least one trumpf.
                            (None, None) => panic!("Two losing players with same points, but none of them with trumpf."),
                        }
                    })
                    .unwrap()
                    .0
            }
        };
        SAccountBalance::new(
            SStossDoublingPayoutDecider::payout(
                EPlayerIndex::map_from_fn(|eplayerindex| {
                    if eplayerindex_loser==eplayerindex {
                        -3 * self.m_n_price
                    } else {
                        self.m_n_price
                    }
                }),
                {
                    assert_eq!(n_stoss, 0); // SRulesRamsch does not allow stoss
                    0
                },
                n_doubling,
            ),
            0,
        )
    }

    fn all_allowed_cards_first_in_stich(&self, _vecstich: &[SStich], hand: &SHand) -> SHandVector {
        hand.cards().clone()
    }

    fn all_allowed_cards_within_stich(&self, vecstich: &[SStich], hand: &SHand) -> SHandVector {
        assert!(!vecstich.is_empty());
        let card_first = *vecstich.last().unwrap().first();
        let veccard_allowed : SHandVector = hand.cards().iter()
            .filter(|&&card| self.trumpforfarbe(card)==self.trumpforfarbe(card_first))
            .cloned()
            .collect();
        if veccard_allowed.is_empty() {
            hand.cards().clone()
        } else {
            veccard_allowed
        }
    }
}
