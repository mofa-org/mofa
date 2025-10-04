import subprocess


def stop_process(process):
    """Terminate one or many subprocess.Popen objects gracefully."""
    processes = process if isinstance(process, list) else [process]
    for proc in processes:
        if proc is None:
            continue
        try:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
        except Exception:
            # Best-effort shutdown; ignore failures so cleanup keeps going
            continue

def stop_dora_dataflow(dataflow_name:str):
    dora_stop_process = subprocess.Popen(
        ['dora', 'stop','--name',dataflow_name],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    stdout, stderr = dora_stop_process.communicate()
    return None


def destroy_dora_daemon():
    """Best-effort wrapper around `dora destroy` to cleanup daemon state."""
    try:
        subprocess.run(
            ['dora', 'destroy'],
            check=False,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except Exception:
        # If destroy fails we silently ignore so CLI shutdown still succeeds
        pass

def send_task_or_stop_process(task:str,dora_dataflow_process,task_input_process,dataflow_name:str):
    if task.lower() in ["exit", "quit"]:
        stop_process([dora_dataflow_process, task_input_process])
        stop_dora_dataflow(dataflow_name=dataflow_name)
        return False
    if task_input_process.poll() is None:
        task_input_process.stdin.write(task + '\n')
        task_input_process.stdin.flush()
        return True

