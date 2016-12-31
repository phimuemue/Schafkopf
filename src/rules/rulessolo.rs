use primitives::*;
use rules::*;
use rules::trumpfdecider::*;
use rules::payoutdecider::*;
use std::fmt;
use std::cmp::Ordering;
use std::marker::PhantomData;

pub struct SRulesSoloLike<TrumpfDecider, PayoutDecider>
    where TrumpfDecider: TTrumpfDecider,
          PayoutDecider: TPayoutDecider,
{
    pub m_str_name: String,
    pub m_eplayerindex : EPlayerIndex, // TODO should be static
    pub m_trumpfdecider : PhantomData<TrumpfDecider>,
    pub m_payoutdecider : PhantomData<PayoutDecider>,
    pub m_prio : VGameAnnouncementPriority,
    m_n_payout_base : isize,
    m_n_payout_schneider_schwarz : isize,
    m_laufendeparams : SLaufendeParams,
}

impl<TrumpfDecider, PayoutDecider> fmt::Display for SRulesSoloLike<TrumpfDecider, PayoutDecider> 
    where TrumpfDecider: TTrumpfDecider,
          PayoutDecider: TPayoutDecider,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.m_str_name)
    }
}

impl<TrumpfDecider, PayoutDecider> TActivelyPlayableRules for SRulesSoloLike<TrumpfDecider, PayoutDecider>
    where TrumpfDecider: TTrumpfDecider,
          TrumpfDecider: Sync,
          PayoutDecider: TPayoutDecider,
          PayoutDecider: Sync,
{
    fn priority(&self) -> VGameAnnouncementPriority {
        self.m_prio.clone()
    }
}

impl<TrumpfDecider, PayoutDecider> TRules for SRulesSoloLike<TrumpfDecider, PayoutDecider> 
    where TrumpfDecider: TTrumpfDecider,
          TrumpfDecider: Sync,
          PayoutDecider: TPayoutDecider,
          PayoutDecider: Sync,
{
    impl_rules_trumpf!(TrumpfDecider);

    fn stoss_allowed(&self, eplayerindex: EPlayerIndex, vecstoss: &[SStoss], hand: &SHand) -> bool {
        assert!(
            vecstoss.iter()
                .enumerate()
                .all(|(i_stoss, stoss)| (i_stoss%2==0) == (stoss.m_eplayerindex!=self.m_eplayerindex))
        );
        assert_eq!(hand.cards().len(), 8);
        (eplayerindex==self.m_eplayerindex)==(vecstoss.len()%2==1)
    }

    fn playerindex(&self) -> Option<EPlayerIndex> {
        Some(self.m_eplayerindex)
    }

    fn payout(&self, gamefinishedstiche: &SGameFinishedStiche, n_stoss: usize, n_doubling: usize, _n_stock: isize) -> SAccountBalance {
        SAccountBalance::new(
            SStossDoublingPayoutDecider::payout(
                PayoutDecider::payout(
                    self,
                    gamefinishedstiche,
                    /*fn_is_player_party*/ |eplayerindex| {
                        eplayerindex==self.m_eplayerindex
                    },
                    /*fn_player_multiplier*/ |eplayerindex| {
                        if self.m_eplayerindex==eplayerindex {
                            3
                        } else {
                            1
                        }
                    },
                    self.m_n_payout_base,
                    self.m_n_payout_schneider_schwarz,
                    &self.m_laufendeparams,
                ),
                n_stoss,
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

impl<TrumpfDecider, PayoutDecider> SRulesSoloLike<TrumpfDecider, PayoutDecider>
    where TrumpfDecider: TTrumpfDecider,
          PayoutDecider: TPayoutDecider,
{
    pub fn new(eplayerindex: EPlayerIndex, prio: VGameAnnouncementPriority, str_rulename: &str, n_payout_base: isize, n_payout_schneider_schwarz: isize, laufendeparams: SLaufendeParams) -> SRulesSoloLike<TrumpfDecider, PayoutDecider> {
        SRulesSoloLike::<TrumpfDecider, PayoutDecider> {
            m_eplayerindex: eplayerindex,
            m_trumpfdecider: PhantomData::<TrumpfDecider>,
            m_payoutdecider: PhantomData::<PayoutDecider>,
            m_prio: prio,
            m_str_name: str_rulename.to_string(),
            m_n_payout_base : n_payout_base,
            m_n_payout_schneider_schwarz : n_payout_schneider_schwarz,
            m_laufendeparams : laufendeparams,
        }
    }
}

pub fn sololike<TrumpfDecider, PayoutDecider>(eplayerindex: EPlayerIndex, prio: VGameAnnouncementPriority, str_rulename: &str, n_payout_base: isize, n_payout_schneider_schwarz: isize, laufendeparams: SLaufendeParams) -> Box<TActivelyPlayableRules> 
    where TrumpfDecider: TTrumpfDecider,
          TrumpfDecider: 'static,
          TrumpfDecider: Sync,
          PayoutDecider: TPayoutDecider,
          PayoutDecider: 'static,
          PayoutDecider: Sync,
{
    Box::new(SRulesSoloLike::<TrumpfDecider, PayoutDecider>::new(eplayerindex, prio, str_rulename, n_payout_base, n_payout_schneider_schwarz, laufendeparams)) as Box<TActivelyPlayableRules>
}

pub type SCoreSolo<TrumpfFarbDecider> = STrumpfDeciderSchlag<
    SSchlagDesignatorOber, STrumpfDeciderSchlag<
    SSchlagDesignatorUnter, TrumpfFarbDecider>>;
pub type SCoreGenericWenz<TrumpfFarbDecider> = STrumpfDeciderSchlag<
    SSchlagDesignatorUnter, TrumpfFarbDecider>;
pub type SCoreGenericGeier<TrumpfFarbDecider> = STrumpfDeciderSchlag<
    SSchlagDesignatorOber, TrumpfFarbDecider>;
