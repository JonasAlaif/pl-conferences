use crate::{conf::AllDates, html, llm};

use super::{Conference, ConferenceAll, ImportantDates};

impl Conference {
    pub async fn search(&self) -> Option<String> {
        let query = format!("{self} conference call for papers cfp");
        let results = html::search(&query).await.unwrap();
        let result = results.get(0)?;
        let model = llm::model().await;

        let r = serde_json::to_string(&result).unwrap();
        let query = format!("{r}\n---\n{}", self.correct_search());
        let correct_page = model.yes_no(&query, true).await.ok()?;
        if !correct_page {
            return None;
        }
        result.scrape().await.ok()
    }

    pub async fn extract(&self, page: &str) -> Option<ConferenceAll> {
        let model = llm::model().await;
        let query = format!("{page}\n---\n{}", self.details());
        let details = model.formatted::<crate::schema::ConferenceDetails>(&query, false)
            .await.ok()?;
        let details = details.into();

        let query = format!("{page}\n---\n{}", self.two_rounds());
        let two_rounds = model.yes_no(&query, true).await.ok()?;

        let query = format!("{page}\n---\n{}", self.summary_table());
        let table = model
            .formatted::<crate::schema::DateTable>(&query, false)
            .await
            .unwrap();

        let dates = if two_rounds {
            let query = format!("{table}\n---\n{}", self.dates());
            let dates = model
                .formatted::<crate::schema::DatesTwoRounds>(&query, false)
                .await.ok()?;
            let dates: (_, _) = dates.into();
            AllDates::TwoRounds(dates.0.check_valid()?, dates.1.check_valid()?)
        } else {
            let query = format!("{table}\n---\n{}", self.dates());
            let dates = model
                .formatted::<crate::schema::DatesOneRound>(&query, false)
                .await.ok()?;
            let dates: ImportantDates = dates.into();
            AllDates::OneRound(dates.check_valid()?)
        };

        Some(ConferenceAll {
            details,
            dates,
        })
    }
}
