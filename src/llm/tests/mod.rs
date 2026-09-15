use crate::{
    conf::{Conference, ConferenceDetails, ImportantDates},
    llm,
};

#[tokio::test]
async fn test_llama3_2() {
    let model = llm::llama3_2().await;
    test_all(model).await;
}

#[tokio::test]
async fn test_gemma_3() {
    let model = llm::gemma_3().await;
    test_all(model).await;
}

#[tokio::test]
async fn test_deepseek_r1() {
    let model = llm::deepseek_r1().await;
    test_all(model).await;
}

async fn test_all(model: llm::Model) {
    // test_yes_no(model).await;
    // test_date(model).await;
    test_conf_page(model).await;
}

async fn test_yes_no(model: llm::Model) {
    model.test_yes_no("Is the sky blue?", false, true).await;
    model.test_yes_no("Is the sky green?", false, false).await;
}

async fn test_date(model: llm::Model) {
    let query = "What day were the twin towers destroyed?";
    model.test_date(query, false, date(2001, 9, 11)).await;
    let query = "If today is 24th August 2022, what date will it be in 10 days time? Do not answer with today's date!";
    model.test_date(query, true, date(2022, 9, 3)).await;
}

fn date(y: i32, m: u32, d: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

async fn test_conf_page(model: llm::Model) {
    let oopsla25 = Conference {
        name: "SPLASH".to_string(),
        track: Some("OOPSLA".to_string()),
        year: 2025,
    };
    let conf = include_str!("oopsla25.md");

    // let query = format!("{conf}\n---\n{}", oopsla25.correct_page());
    // model.test_yes_no(&query, true, true).await;

    let query = format!("{conf}\n---\nThe above document is about '{oopsla25}'. Find all the required information about this conference.");
    model
        .test_formatted::<crate::schema::ConferenceDetails>(&query, false, |details| {
            let details = ConferenceDetails::from(details);
            assert_eq!(details.start, date(2025, 10, 12));
            assert_eq!(details.end, date(2025, 10, 18));
            assert_eq!(details.city, "Singapore");
            assert_eq!(details.country, "Singapore");
        })
        .await;

    let query = format!("{conf}\n---\n{}", oopsla25.two_rounds());
    model.test_yes_no(&query, true, true).await;

    let query = format!("{conf}\n---\nThe above document is about '{oopsla25}'. Summarize all the dates from this document. Include **all** dates and ranges/durations in the document. All must have a proper description. You must not make any mistakes, otherwise you will be severely punished.");
    let table = model
        .formatted::<crate::schema::DateTable>(&query, false)
        .await
        .unwrap();

    let query = format!("{table}\n---\nThe list above contains all important dates for '{oopsla25}'. Summarize the dates in this list as a json structure.");
    model
        .test_formatted::<crate::schema::DatesTwoRounds>(&query, false, |data| {
            let (round_1, round_2) = <(ImportantDates, ImportantDates)>::from(data);

            assert_eq!(round_1.paper_submission, date(2024, 10, 15));
            assert_eq!(round_1.author_response_start, date(2024, 12, 3));
            assert_eq!(round_1.author_response_end, date(2024, 12, 6));
            assert_eq!(round_1.notification, date(2024, 12, 18));

            assert_eq!(round_2.paper_submission, date(2025, 3, 25));
            assert_eq!(round_2.author_response_start, date(2025, 5, 26));
            assert_eq!(round_2.author_response_end, date(2025, 5, 29));
            assert_eq!(round_2.notification, date(2025, 6, 18));
        })
        .await;
}
