from langchain_core.output_parsers import JsonOutputParser


def json_output_openai_result(data:str):
    output_parser = JsonOutputParser()
    return output_parser.parse(data)