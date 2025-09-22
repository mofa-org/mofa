import os
from typing import Union


def init_proxy_env(is_proxy:bool=True, proxy_url:str= 'http://192.168.0.75:10809', ):
    if is_proxy:
        os.environ['http_proxy'] = proxy_url
        os.environ['https_proxy'] = proxy_url


def init_env(env:dict):
    for env_name, env_value in env.items():
        os.environ[env_name] = env_value


def set_api_keys(
    model_api_key: str,
    module_api_url: Union[str] = None,
    model_name: Union[str] = None,
    model_max_tokens: Union[int,str] = None
) -> None:
    os.environ["OPENAI_API_KEY"] = model_api_key
    if module_api_url:
        os.environ["OPENAI_API_BASE"] = module_api_url
    if model_name:
        os.environ["OPENAI_MODEL_NAME"] = model_name
    if model_max_tokens:
        os.environ["OPENAI_MAX_TOKENS"] = str(20480)
