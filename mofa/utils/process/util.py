import subprocess


def stop_process(process):
    if isinstance(process,list):
        for p in process:
            p.terminate()
    else:
        process.terminate()

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

def send_task_or_stop_process(task:str,dora_dataflow_process,task_input_process,dataflow_name:str):
    if task.lower() in ["exit", "quit"]:
        stop_process([dora_dataflow_process, task_input_process])
        stop_dora_dataflow(dataflow_name=dataflow_name)
        return False
    if task_input_process.poll() is None:
        task_input_process.stdin.write(task + '\n')
        task_input_process.stdin.flush()
        return True