"""
Reproduce the fuzzy_lookup date-stripping logic from
mofa-foundation/src/cost/pricing.rs (lines 92-103)
"""

def strip_date_suffix(model_lower):
    parts = model_lower.split('-')
    kept = []
    for part in parts:
        try:
            int(part)
            is_err = False
        except ValueError:
            is_err = True
        if is_err or len(part) < 4:
            kept.append(part)
        else:
            break
    return '-'.join(kept)

REGISTRY = {
    "openai/gpt-4o",
    "openai/gpt-4o-mini",
    "openai/gpt-4-turbo",
    "openai/gpt-3.5-turbo",
    "openai/o1",
    "openai/o1-mini",
    "anthropic/claude-3.5-sonnet",
    "anthropic/claude-3-haiku",
    "anthropic/claude-3-opus",
    "anthropic/claude-3.5-haiku",
}

def fuzzy_lookup(provider, model):
    provider_lower = provider.lower()
    model_lower = model.lower()
    exact_key = f"{provider_lower}/{model_lower}"
    if exact_key in REGISTRY:
        return f"FOUND (exact)"
    base_model = strip_date_suffix(model_lower)
    if base_model != model_lower:
        base_key = f"{provider_lower}/{base_model}"
        if base_key in REGISTRY:
            return f"FOUND (stripped '{model_lower}'->'{base_model}')"
        else:
            return f"NOT FOUND (stripped '{model_lower}'->'{base_model}', tried '{base_key}')"
    return f"NOT FOUND (not stripped, model='{model_lower}')"

print("Testing fuzzy_lookup date-suffix stripping from pricing.rs")
print("=" * 65)

test_cases = [
    ("openai",    "gpt-4o-2024-05-13"),
    ("openai",    "gpt-4-turbo-2024-04-09"),
    ("openai",    "gpt-3.5-turbo-0125"),
    ("anthropic", "claude-3-opus-20240229"),
    ("anthropic", "claude-3.5-sonnet-20241022"),
    ("openai",    "o1-mini-2024-09-12"),
    ("openai",    "gpt-4o"),
    ("openai",    "gpt-4o-mini"),
]

for provider, model in test_cases:
    result = fuzzy_lookup(provider, model)
    print(f"  {provider}/{model:<42} => {result}")

print()
print("Per-segment trace for 'gpt-3.5-turbo-0125':")
for part in "gpt-3.5-turbo-0125".split('-'):
    try:
        int(part); is_err = False
    except ValueError:
        is_err = True
    keep = is_err or len(part) < 4
    print(f"  part={part!r:8}  is_err={is_err}  len={len(part)}  keep={keep}")
print(f"  => stripped: {strip_date_suffix('gpt-3.5-turbo-0125')!r}")

print()
print("Per-segment trace for 'claude-3-opus-20240229':")
for part in "claude-3-opus-20240229".split('-'):
    try:
        int(part); is_err = False
    except ValueError:
        is_err = True
    keep = is_err or len(part) < 4
    print(f"  part={part!r:12}  is_err={is_err}  len={len(part)}  keep={keep}")
print(f"  => stripped: {strip_date_suffix('claude-3-opus-20240229')!r}")
