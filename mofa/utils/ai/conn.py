import os
from typing import List
from dotenv import load_dotenv
try:
    import instructor
except ImportError:
    instructor = None

try:
    from openai import OpenAI
except ImportError:
    OpenAI = None


def create_openai_client(api_key: str = None, env_file: str = None, *args, **kwargs):
    if OpenAI is None:
        raise ImportError("openai package is required to create an OpenAI client.")

    env_file = env_file or os.getenv("ENV_FILE", ".env.secret")
    load_dotenv(env_file)

    if api_key is not None:
        resolved_api_key = api_key
    elif os.getenv("LLM_API_KEY"):
        resolved_api_key = os.getenv("LLM_API_KEY")
    else:
        resolved_api_key = os.getenv("OPENAI_API_KEY")

    if not resolved_api_key:
        raise RuntimeError("No API key found. Set OPENAI_API_KEY or LLM_API_KEY.")

    base_url = os.getenv("LLM_BASE_URL", None)
    if base_url is None:
        client = OpenAI(api_key=resolved_api_key, **kwargs)
    else:
        client = OpenAI(api_key=resolved_api_key, base_url=base_url, **kwargs)
    return client
def generate_json_from_llm(client, prompt: str, format_class, messages: List[dict] = None, supplement_prompt: str = None, model_name: str = 'gpt-4o-mini') -> str:

    if messages is None:
        messages = [
            {"role": "system",
             "content": "You are a professional Ai assistant"},
            {"role": "user", "content": prompt},
        ]
    if supplement_prompt is not None:
        messages.append({"role": "user", "content": supplement_prompt})
    completion = client.beta.chat.completions.parse(
        model=model_name,
        messages=messages,
        response_format=format_class,
    )
    return completion.choices[0].message.parsed

def structor_llm(env_file:str,messages:list,response_model,model_name:str='gpt-4o',*args,**kwargs):

    if OpenAI is None or instructor is None:
        raise ImportError("openai and instructor packages are required for structor_llm.")

    load_dotenv(env_file)
    max_loop = 3
    if os.getenv('LLM_API_KEY') is not None:
        os.environ['OPENAI_API_KEY'] = os.getenv('LLM_API_KEY')

    if os.getenv('LLM_BASE_URL',None) is None:
        client = instructor.from_openai(client=OpenAI(api_key=os.environ['OPENAI_API_KEY']))
    else:
        client = instructor.from_openai(client=OpenAI(api_key=os.environ['OPENAI_API_KEY'],base_url=os.getenv('LLM_BASE_URL'),))
    try:
        print('llm_model_name -> ',os.getenv('LLM_MODEL_NAME','gpt-4o'))
        response = client.chat.completions.create(
            model=os.getenv('LLM_MODEL_NAME','gpt-4o'),
            messages=messages,
            response_model=response_model,
        )
    except Exception as e:
        print(f"Error: {e}")
        print('Error Messagens -> ',messages,  "Error Model Name -> ",model_name)
        response = None
        for i in range(max_loop):
            try:
                response = client.chat.completions.create(
                    model=os.getenv('LLM_MODEL_NAME','gpt-4o'),
                    messages=messages,
                    response_model=response_model,
                )
                break
            except Exception as e:
                print(f"Retrying... {i+1}/{max_loop} - Error: {e}")
    return response

