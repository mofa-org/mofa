from mofa.kernel.utils.log import write_agent_log
from mofa.utils.files.read import read_yaml


def record_agent_prompt_log(agent_config:dict, config_file_path:str, log_key_name:str= 'Agent Prompt', task:str=None):
    log_config = {}
    agent_prompt_config = read_yaml(config_file_path).get('AGENT', '')
    if task is not None:
        agent_prompt_config['task'] = task
    log_config[log_key_name] = agent_prompt_config
    write_agent_log(log_type=agent_config.get('log_type', 'md'), log_file_path=agent_config.get('log_path', None),
                    data=log_config)



def record_agent_result_log(agent_config:dict,agent_result:dict,):
    write_agent_log(log_type=agent_config.get('log_type', 'md'), log_file_path=agent_config.get('log_path', None),
                    data=agent_result)
