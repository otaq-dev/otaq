//! The M12 states and flags (T-0182; SDD §7.2, §12.2, §13; d0063, d0064, d0042, d0016): needs-you ones group like
//! the needs-you family, Validating holds one slot and makes no inbox item, and the ticket columns become the
//! card's flags.

use std::collections::HashSet;

use crate::common::Fixture;
use otaq_core::board::needs_you::{NeedsYouGroup, group_of};
use otaq_core::domain::{ReviewEntry, Status, ValidationRound};
use otaq_core::scheduler::{HOLDS_A_SLOT, running_count};

const M12_DEV: [Status; 6] = [
    Status::Validating,
    Status::ValidatorNeedsYou,
    Status::ValidationFailed,
    Status::ValidationNoResult,
    Status::MergeConflictFixing,
    Status::AutoMergeStopped,
];

#[test]
fn needs_you_states_are_inbox_group_one_and_validating_makes_no_item() {
    for status in M12_DEV {
        let expected = status
            .is_needs_you_family()
            .then_some(NeedsYouGroup::Family);
        assert_eq!(group_of(status), expected, "{status}");
    }
    assert_eq!(group_of(Status::Validating), None);
    assert_eq!(group_of(Status::MergeConflictFixing), None);
}

#[test]
fn every_m12_dev_state_holds_one_slot_per_ticket() {
    for status in M12_DEV {
        assert!(HOLDS_A_SLOT.contains(&status), "{status}");
    }
    assert_eq!(
        HOLDS_A_SLOT
            .iter()
            .filter(|s| **s == Status::Validating)
            .count(),
        1
    );
}

#[tokio::test]
async fn a_validating_ticket_counts_once_toward_the_limit() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout").await;
    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'validating', board_column = 'dev' WHERE id = ?",
    )
    .await;
    let tickets = f.board.tickets("SHOP").await.unwrap();
    assert_eq!(running_count(&tickets, &HashSet::new()), 1);
}

async fn set(f: &Fixture, id: &str, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .execute(f.board.store().pool())
        .await
        .unwrap();
}

/// The Review entry shows in Review; the round marker on a working ticket after a failed round, with the
/// project's rounds (plus extra ones); Merged automatically on Done.
#[tokio::test]
async fn ticket_columns_become_flags() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout").await;
    let key = t.key.to_string();

    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'working', board_column = 'dev', validation_round = 1 WHERE id = ?",
    )
    .await;
    let ticket = f.ticket(&key).await;
    assert_eq!(
        ticket.flags.validation_round,
        Some(ValidationRound { round: 1, of: 3 })
    );
    assert_eq!(
        ticket.flags.validation_round.unwrap().to_string(),
        "Validation failed · round 1 of 3"
    );
    set(
        &f,
        &t.id,
        "UPDATE tickets SET validation_rounds_extra = 1 WHERE id = ?",
    )
    .await;
    assert_eq!(
        f.ticket(&key).await.flags.validation_round,
        Some(ValidationRound { round: 1, of: 4 }),
        "Give it another round allows one more"
    );
    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'validating' WHERE id = ?",
    )
    .await;
    assert_eq!(f.ticket(&key).await.flags.validation_round, None);

    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'ready_for_review', board_column = 'ready_for_review', \
         review_entry = 'validated', validation_round = 2 WHERE id = ?",
    )
    .await;
    let ticket = f.ticket(&key).await;
    let entry = ticket.flags.review_entry.unwrap();
    assert_eq!(entry.entry, ReviewEntry::Validated);
    assert_eq!(entry.to_string(), "Validated · round 2");
    assert_eq!(ticket.flags.validation_round, None);

    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'merged', board_column = 'done', merged_automatically = TRUE WHERE id = ?",
    )
    .await;
    let ticket = f.ticket(&key).await;
    assert!(ticket.flags.merged_automatically);
    assert_eq!(ticket.column, otaq_core::domain::Column::Done);
}

/// The card's details (T-0192): the round the validator checks, the last failed round's findings (notes left
/// out), and the files a conflict touches.
#[tokio::test]
async fn validating_round_findings_and_conflict_files() {
    let f = Fixture::shop().await;
    let t = f.create("Checkout").await;
    let key = t.key.to_string();

    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'validating', board_column = 'dev', validation_round = 1 WHERE id = ?",
    )
    .await;
    assert_eq!(
        f.ticket(&key).await.flags.validating_round,
        Some(ValidationRound { round: 2, of: 3 }),
        "the second round is being checked"
    );
    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'validator_needs_you' WHERE id = ?",
    )
    .await;
    assert!(f.ticket(&key).await.flags.validating_round.is_some());

    set(
        &f,
        &t.id,
        "INSERT INTO validations (id, ticket_id, round, trigger, started_at, ended_at, outcome, report) \
         VALUES ('v1', ?, 1, 'agent', 1, 2, 'fail', \
                 '- src/cart.ts:12 the total ignores the coupon\n- Note: covered by SHOP-40.2\n\n- no test for it')",
    )
    .await;
    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'working' WHERE id = ?",
    )
    .await;
    let ticket = f.ticket(&key).await;
    assert_eq!(ticket.flags.validating_round, None);
    assert_eq!(
        ticket.flags.findings,
        [
            "src/cart.ts:12 the total ignores the coupon",
            "no test for it"
        ]
    );
    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'validation_failed' WHERE id = ?",
    )
    .await;
    assert_eq!(f.ticket(&key).await.flags.findings.len(), 2);
    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'ready_for_review', board_column = 'ready_for_review', \
         review_entry = 'failed_moved' WHERE id = ?",
    )
    .await;
    assert!(
        f.ticket(&key).await.flags.findings.is_empty(),
        "only while the findings matter on the card"
    );

    set(
        &f,
        &t.id,
        "UPDATE tickets SET state = 'merge_conflict_fixing', board_column = 'dev', review_entry = NULL, \
         conflicts_with = 'main', conflict_files = '[\"lib/wishlist.ts\"]' WHERE id = ?",
    )
    .await;
    assert_eq!(
        f.ticket(&key).await.flags.conflict_files,
        ["lib/wishlist.ts"]
    );
}
