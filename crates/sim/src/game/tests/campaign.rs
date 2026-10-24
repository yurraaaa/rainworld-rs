use super::*;

#[test]
fn base_campaigns() {
    let mods = ActiveMods::NONE;
    let names: Vec<_> = SlugcatCampaign::all(mods).iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        [SlugcatName::White, SlugcatName::Yellow, SlugcatName::Red]
    );
    let monk = SlugcatCampaign::of(SlugcatName::Yellow, mods).unwrap();
    assert_eq!((monk.stats.max_food, monk.stats.food_to_hibernate), (5, 3));
    assert_eq!(monk.start_den, "SU_C04");
    let hunter = SlugcatCampaign::of(SlugcatName::Red, mods).unwrap();
    assert_eq!(hunter.start_region(), "LF");
    assert_eq!(hunter.stats.malnourished().food_to_hibernate, 9);
    assert!(SlugcatCampaign::of(SlugcatName::Night, mods).is_none());
    // The malnourished branch of the `SlugcatStats` constructor.
    for c in SlugcatCampaign::all(mods) {
        let s = c.stats.malnourished();
        assert_eq!(s.food_to_hibernate, s.max_food);
        assert_eq!(s.throwing_skill, 0);
        // `min(bodyWeightFac, 0.9)` of 1, 0.95 and 1.12.
        assert_eq!(s.body_weight_fac, 0.9);
        assert_eq!(s.lungs_fac, c.stats.lungs_fac);
        assert_eq!(
            (
                s.runspeed_fac,
                s.pole_climb_speed_fac,
                s.corridor_climb_speed_fac
            ),
            (0.875, 0.8, 0.86)
        );
        assert!(s.malnourished);
    }
    for c in SlugcatCampaign::all(mods) {
        assert_eq!(c.name.timeline(), c.timeline);
    }
    assert_eq!(player_start_pos("SU_C04", mods), Some((7, 28)));
    assert_eq!(player_start_pos("LF_H01", mods), Some((350, 13)));
    assert_eq!(player_start_pos("SU_A41", mods), None);
}
