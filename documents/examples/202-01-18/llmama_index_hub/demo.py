from llama_index.readers.web import BeautifulSoupWebReader

loader = BeautifulSoupWebReader()
documents = loader.load_data(urls=["https://langchain-ai.github.io/langgraph/"])
print(documents)