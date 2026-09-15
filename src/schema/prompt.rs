macro_rules! prompt {
    ($struct:ident => $prompt:literal) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $struct;

        impl schemars::JsonSchema for $struct {
            fn schema_name() -> String {
                stringify!($struct).to_owned()
            }

            fn json_schema(
                generator: &mut schemars::r#gen::SchemaGenerator,
            ) -> schemars::schema::Schema {
                let mut schema =
                    <String as schemars::JsonSchema>::json_schema(generator).into_object();
                schema.const_value = Some(serde_json::Value::String($prompt.to_owned()));
                schema.into()
            }
        }

        impl<'de> serde::Deserialize<'de> for $struct {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                String::deserialize(deserializer).map(|_| $struct)
            }
        }
    };
}
