use super::*;

/// `SocialEventRecognizer.WeaponAttack` and `LizardAI.SocialEvent`.
#[test]
fn weapon_attack_events() {
    assert_eq!(
        SocialEventId::weapon_attack(true, true),
        SocialEventId::LethalAttack
    );
    assert_eq!(
        SocialEventId::weapon_attack(true, false),
        SocialEventId::LethalAttackAttempt
    );
    assert_eq!(
        SocialEventId::weapon_attack(false, true),
        SocialEventId::NonLethalAttack
    );
    assert_eq!(
        SocialEventId::weapon_attack(false, false),
        SocialEventId::NonLethalAttackAttempt
    );
    assert_eq!(SocialEventId::NonLethalAttackAttempt.lizard_offence(), 0.05);
    assert_eq!(SocialEventId::NonLethalAttack.lizard_offence(), 0.15);
    assert_eq!(SocialEventId::LethalAttackAttempt.lizard_offence(), 0.35);
    assert_eq!(SocialEventId::LethalAttack.lizard_offence(), 1.0);
    assert_eq!(SocialEventId::Killing.lizard_offence(), 0.0);
    assert_eq!(
        SocialEventId::from_value("ItemTransaction"),
        Some(SocialEventId::ItemTransaction)
    );
}
