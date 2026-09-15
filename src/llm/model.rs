use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::OnceCell;

use ollama_rs::{
    generation::{
        completion::request::GenerationRequest,
        parameters::{FormatType, JsonStructure},
    },
    models::{create::CreateModelRequest, ModelOptions},
    Ollama,
};

use super::ollama;

pub async fn llama3_2() -> Model {
    static MODEL: OnceCell<Model> = OnceCell::const_new();
    *MODEL.get_or_init(|| Model::new("llama3.2")).await
}

pub async fn gemma_3() -> Model {
    static MODEL: OnceCell<Model> = OnceCell::const_new();
    *MODEL.get_or_init(|| Model::new("gemma3")).await
}

pub async fn deepseek_r1() -> Model {
    static MODEL: OnceCell<Model> = OnceCell::const_new();
    *MODEL.get_or_init(|| Model::new("deepseek-r1")).await
}

#[derive(Clone, Copy)]
pub struct Model {
    pub(super) name: &'static str,
    ollama: &'static Ollama,
}

impl Model {
    pub async fn new(name: &'static str) -> Self {
        let self_ = Self {
            name,
            ollama: ollama(),
        };
        self_.init().await;
        self_
    }

    pub async fn yes_no(self, query: &str, reasoning: bool) -> ollama_rs::error::Result<bool> {
        self.formatted::<YesNo>(query, reasoning)
            .await
            .map(|yn| matches!(yn, YesNo::Yes))
    }

    pub async fn date(self, query: &str, reasoning: bool) -> ollama_rs::error::Result<NaiveDate> {
        self.formatted::<crate::schema::Date>(query, reasoning)
            .await
            .map(|d| d.as_naive_date())
    }

    pub async fn formatted<T: JsonSchema + for<'a> Deserialize<'a>>(
        self,
        query: &str,
        reasoning: bool,
    ) -> ollama_rs::error::Result<T> {
        let model = format!("{}:cold", self.name);
        self.formatted_inner(model, query, reasoning).await
    }

    async fn formatted_inner<T: JsonSchema + for<'a> Deserialize<'a>>(
        self,
        model: String,
        query: &str,
        reasoning: bool,
    ) -> ollama_rs::error::Result<T> {
        let prompt = query.to_string();

        #[derive(JsonSchema, Deserialize)]
        #[allow(dead_code)]
        struct Reasoning<T> {
            #[schemars(description = "My thinking about how to answer.")]
            reasoning: String,
            answer: T,
        }
        let f = if reasoning {
            JsonStructure::new::<Reasoning<T>>()
        } else {
            JsonStructure::new::<T>()
        };
        let f = FormatType::StructuredJson(f);
        let request = GenerationRequest::new(model, prompt).format(f);
        eprintln!("[Q] {}", query.split("---\n").last().unwrap());
        eprint!("[A] ");
        let response = self.stream_request(request).await?;
        eprintln!("\n");
        let response = if reasoning {
            serde_json::from_str::<Reasoning<T>>(&response).map(|r| r.answer)
        } else {
            serde_json::from_str::<T>(&response)
        };
        response.map_err(Into::into)
    }

    async fn stream_request(
        self,
        request: GenerationRequest<'_>,
    ) -> ollama_rs::error::Result<String> {
        use tokio_stream::StreamExt;
        let mut response = String::new();
        let mut stream = self.ollama.generate_stream(request).await?;
        while let Some(res) = stream.next().await {
            for chunk in res? {
                response += &chunk.response;
                eprint!("{}", chunk.response.replace('\n', " "));
            }
        }
        Ok(response)
    }

    async fn init(self) {
        let model = self.name;

        let models = self.ollama.list_local_models().await.unwrap();
        let models = models.into_iter().map(|m| m.name).collect::<Vec<_>>();
        println!("Local {:?}", models);
        if models.iter().all(|m| !m.starts_with(model)) {
            use tokio_stream::StreamExt;
            let mut stream = self
                .ollama
                .pull_model_stream(model.to_string(), false)
                .await
                .unwrap();
            while let Some(next) = stream.next().await {
                let _next = next.unwrap();
                eprint!(".");
            }
            eprintln!();
        }

        let system = "You are a highly skilled document analyser with decades of experience. You answer all requests with the highest level of accuracy and detail, always referencing the source document. You have never and will never make a mistake. You are extremely confident in your work.";

        let parameters = ModelOptions::default()
            .temperature(0.5)
            .top_p(0.5)
            .num_predict(25000)
            .num_ctx(25000);
        let warm = CreateModelRequest::new(format!("{model}:warm"))
            .from_model(model.to_string())
            .parameters(parameters)
            .system(system.to_string());
        self.ollama.create_model(warm).await.unwrap();

        let parameters = ModelOptions::default()
            .temperature(0.)
            .top_p(0.1)
            .top_k(5)
            .num_predict(25000)
            .num_ctx(25000);
        let cold = CreateModelRequest::new(format!("{model}:cold"))
            .from_model(model.to_string())
            .parameters(parameters)
            .system(system.to_string());
        self.ollama.create_model(cold).await.unwrap();
    }

    pub(super) async fn test_yes_no(&self, query: &str, reasoning: bool, expected: bool) {
        self.test_formatted::<YesNo>(query, reasoning, |yn| {
            assert_eq!(matches!(yn, YesNo::Yes), expected)
        })
        .await
    }

    pub(super) async fn test_date(&self, query: &str, reasoning: bool, expected: NaiveDate) {
        self.test_formatted::<crate::schema::Date>(query, reasoning, |d| {
            assert_eq!(d.as_naive_date(), expected)
        })
        .await
    }

    pub(super) async fn test_formatted<T: JsonSchema + for<'a> Deserialize<'a>>(
        &self,
        query: &str,
        reasoning: bool,
        check: impl Fn(T),
    ) {
        let resp = self
            .formatted_inner(format!("{}:cold", self.name), query, reasoning)
            .await
            .unwrap();
        check(resp);
        for _ in 0..10 {
            let resp = self
                .formatted_inner(format!("{}:warm", self.name), query, reasoning)
                .await
                .unwrap();
            check(resp);
        }
    }
}

#[derive(JsonSchema, Deserialize)]
enum YesNo {
    Yes,
    No,
}
