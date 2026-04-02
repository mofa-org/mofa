import re
import os

def fix_let_await(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    # match: let <var> = <expr>.await;
    # but ONLY if the next line uses .is_err(), .is_ok(), etc.
    # Actually, simpler: replace all `let var = xxx.await;` that are causing issues
    # Let's just blindly replace `let (\w+) = (.*?\.(?:infer|send_and_capture|last_call|nth_call).*?)\.await;`
    # with `let \1: Result<_, _> = \2.await;`
    # Wait, `last_call()` returns `Option<_>`.
    
    # Just fix the specific patterns causing issues
    # For .infer() and .send_and_capture():
    content = re.sub(r'let (\w+) = (.*?\.(?:infer|send_and_capture|execute)\(.*?\))\.await;', r'let \1: Result<_, _> = \2.await;', content)
    
    # For last_call() and nth_call() and get_session/get_events/list_sessions:
    content = re.sub(r'let (\w+) = (.*?\.(?:last_call|nth_call)\(.*?\))\.await;', r'let \1: Option<_> = \2.await;', content)
    
    content = re.sub(r'let (\w+) = (.*?\.(?:get_session|get_events|list_sessions)\(.*?\))\.await;', r'let \1: Result<_, _> = \2.await;', content)

    # For load_mcp_server
    content = re.sub(r'let (\w+) = (.*?(?:load_mcp_server)\(.*?\))\.await;', r'let \1: Result<_, _> = \2.await;', content)

    # For builder patterns on TestReportBuilder:
    # let report = TestReportBuilder::... .await;
    # We can just change `let report = ` to `let report: Result<_, _> = `
    content = re.sub(r'let report = (TestReportBuilder.*\.await);', r'let report: Result<_, _> = \1;', content, flags=re.DOTALL)

    if content != open(filepath, 'r', encoding='utf-8').read():
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"Fixed types in {filepath}")

for root, dirs, files in os.walk('tests'):
    for file in files:
        if file.endswith('.rs'):
            fix_let_await(os.path.join(root, file))

for root, dirs, files in os.walk('crates'):
    for file in files:
        if file.endswith('.rs'):
            fix_let_await(os.path.join(root, file))
