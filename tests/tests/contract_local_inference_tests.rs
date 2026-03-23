use mofa_foundation::orchestrator::traits::ModelProvider;
use mofa_local_llm::{ComputeBackend, LinuxInferenceConfig, LinuxLocalProvider};
use mofa_testing::{assert_contains_all, assert_error_contains, assert_exact_match, load_fixture};
use serde::Deserialize;
use tempfile::NamedTempFile;

#[derive(Debug, Deserialize)]
struct LocalInferenceContractFixture {
    case_name: String,
    kind: Option<String>,
    target: Option<String>,
    model_name: String,
    model_path: Option<String>,
    backend: String,
    input: Option<String>,
    #[serde(default)]
    create_temp_model: bool,
    expected_error_contains: Option<String>,
    #[serde(default)]
    expected_output_contains: Vec<String>,
    #[serde(default)]
    expected_metadata_fields: Vec<String>,
    #[serde(default)]
    deterministic: bool,
}

fn parse_backend(raw: &str) -> ComputeBackend {
    match raw {
        "cpu" => ComputeBackend::Cpu,
        "cuda" => ComputeBackend::Cuda,
        "rocm" => ComputeBackend::Rocm,
        "vulkan" => ComputeBackend::Vulkan,
        other => panic!("unsupported backend fixture value: {other}"),
    }
}

#[tokio::test]
async fn local_inference_contract_infer_before_load() {
    let fixture: LocalInferenceContractFixture =
        load_fixture("contracts/local_inference/infer_before_load.yaml").expect("fixture");
    assert_eq!(fixture.kind.as_deref(), Some("contract"));
    assert_eq!(fixture.target.as_deref(), Some("local-inference"));
    assert_eq!(fixture.case_name, "local-inference-before-load");

    let provider = LinuxLocalProvider::new(
        LinuxInferenceConfig::new(&fixture.model_name, fixture.model_path.unwrap_or_default())
            .with_backend(parse_backend(&fixture.backend)),
    )
    .expect("provider creation");

    let err = provider
        .infer(fixture.input.as_deref().unwrap_or("hello"))
        .await
        .expect_err("infer before load must fail");
    assert_error_contains(
        err.to_string(),
        fixture
            .expected_error_contains
            .as_deref()
            .expect("expected error substring"),
    );
}

#[tokio::test]
async fn local_inference_contract_missing_model_path() {
    let fixture: LocalInferenceContractFixture =
        load_fixture("contracts/local_inference/missing_model_path.json").expect("fixture");
    assert_eq!(fixture.case_name, "local-inference-missing-model");

    let mut provider = LinuxLocalProvider::new(
        LinuxInferenceConfig::new(
            &fixture.model_name,
            fixture.model_path.clone().expect("model path"),
        )
        .with_backend(parse_backend(&fixture.backend)),
    )
    .expect("provider creation");

    let err = provider
        .load()
        .await
        .expect_err("missing model path must fail");
    assert_error_contains(
        err.to_string(),
        fixture
            .expected_error_contains
            .as_deref()
            .expect("expected error substring"),
    );
}

#[tokio::test]
async fn local_inference_contract_successful_load_and_determinism() {
    let fixture: LocalInferenceContractFixture =
        load_fixture("contracts/local_inference/successful_load.yaml").expect("fixture");
    assert_eq!(fixture.case_name, "local-inference-success");

    let temp_model = if fixture.create_temp_model {
        let file = NamedTempFile::new().expect("temp model file");
        std::fs::write(file.path(), b"GGUF").expect("write temp model");
        Some(file)
    } else {
        None
    };

    let model_path = temp_model
        .as_ref()
        .map(|file| file.path().to_string_lossy().to_string())
        .or(fixture.model_path.clone())
        .expect("model path");

    let mut provider = LinuxLocalProvider::new(
        LinuxInferenceConfig::new(&fixture.model_name, model_path)
            .with_backend(parse_backend(&fixture.backend)),
    )
    .expect("provider creation");

    provider.load().await.expect("provider should load");

    let input = fixture.input.as_deref().unwrap_or("hello world");
    let first = provider.infer(input).await.expect("first inference");
    assert_contains_all(&first, &fixture.expected_output_contains);

    let metadata = provider.get_metadata();
    for key in &fixture.expected_metadata_fields {
        assert!(metadata.contains_key(key), "missing metadata key '{key}'");
    }

    if fixture.deterministic {
        let second = provider.infer(input).await.expect("second inference");
        assert_exact_match(&first, &second);
    }
}
