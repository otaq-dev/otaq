//! Answered `ask human` questions are listed for the ticket's Messages section ("Logged in SHOP-21 Messages", *10 →
//! Answer · inline*); open ones are not.

use otaq_core::service::testing::TestService;

#[tokio::test(flavor = "multi_thread")]
async fn answered_questions_are_listed_oldest_first() {
    let w = TestService::start().await;
    w.add_project("shop-web").await;
    let t = w.create("SHOP", "Order confirmation email").await;
    let first = w
        .board
        .store_question(
            &t.id,
            "Which email provider?",
            &["Postmark".into(), "SES".into()],
            Some(1),
        )
        .await
        .unwrap();
    w.board
        .record_answer(&first.id, "Answer to your question: Postmark (option 1).")
        .await
        .unwrap();
    w.board
        .store_question(&t.id, "Still open?", &[], None)
        .await
        .unwrap();
    let answered = w.board.answered_human_questions(&t.id).await.unwrap();
    assert_eq!(answered.len(), 1);
    assert_eq!(answered[0].question, "Which email provider?");
    assert_eq!(
        answered[0].answer,
        "Answer to your question: Postmark (option 1)."
    );
    assert!(answered[0].answered_at >= answered[0].asked_at);
    w.stop().await;
}
