"""
This file bundles language identification functions.

Modifications (fork): Copyright (c) 2021, Adrien Barbaresi.

Original code: Copyright (c) 2011 Marco Lui <saffsd@gmail.com>.
Based on research by Marco Lui and Tim Baldwin.

See LICENSE file for more info.
https://github.com/adbar/py3langid

Projects:
https://github.com/juntaosun/LangSegment
"""

import re
from collections import defaultdict

# import langid
import py3langid as langid
# pip install py3langid==0.2.2


# -----------------------------------
# 更新日志：新版本分词更加精准。
# Changelog: The new version of the word segmentation is more accurate.
# -----------------------------------


# Word segmentation function:
# automatically identify and split the words (Chinese/English/Japanese/Korean) in the article or sentence according to different languages,
# making it more suitable for TTS processing.
# This code is designed for front-end text multi-lingual mixed annotation distinction, multi-language mixed training and inference of various TTS projects.
# This processing result is mainly for (Chinese = zh, Japanese = ja, English = en, Korean = ko), and can actually support up to 97 different language mixing processing.


# ===========================================================================================================
# 分词功能：将文章或句子里的例如（中/英/日/韩），按不同语言自动识别并拆分，让它更适合TTS处理。
# 本代码专为各种 TTS 项目的前端文本多语种混合标注区分，多语言混合训练和推理而编写。
# ===========================================================================================================
# （1）自动分词：“韩语中的오빠读什么呢？あなたの体育の先生は誰ですか? 此次发布会带来了四款iPhone 15系列机型”
# （2）手动分词：“你的名字叫<ja>佐々木？<ja>吗？”
# 本处理结果主要针对（中文=zh , 日文=ja , 英文=en , 韩语=ko）, 实际上可支持多达 97 种不同的语言混合处理。
# ===========================================================================================================


# 手动分词标签规范：<语言标签>文本内容</语言标签>
# Manual word segmentation tag specification: <language tags> text content </language tags>
# ===========================================================================================================
# For manual word segmentation, labels need to appear in pairs, such as:
# 如需手动分词，标签需要成对出现，例如：“<ja>佐々木<ja>”  或者  “<ja>佐々木</ja>”
# 错误示范：“你的名字叫<ja>佐々木。” 此句子中出现的单个<ja>标签将被忽略，不会处理。
# Error demonstration: "Your name is <ja>佐々木。" Single <ja> tags that appear in this sentence will be ignored and will not be processed.
# ===========================================================================================================

class LangSegment():

    _text_cache = None
    _text_lasts = None
    _text_langs = None
    _lang_count = None
    _lang_eos =   None

    # 可自定义语言匹配标签：
    # Customizable language matching tags: These are supported
    # <zh>你好<zh> , <ja>佐々木</ja> , <en>OK<en> , <ko>오빠</ko> 这些写法均支持
    SYMBOLS_PATTERN = r'(<([a-zA-Z|-]*)>(.*?)<\/*[a-zA-Z|-]*>)'

    # 语言过滤组功能, 可以指定保留语言。不在过滤组中的语言将被清除。您可随心搭配TTS语音合成所支持的语言。
    # The language filter group function allows you to specify reserved languages.
    # Languages not in the filter group will be cleared. You can match the languages supported by TTS Text To Speech as you like.
    Langfilters = ["zh", "en", "ja", "ko"]
    # 除此以外，它支持简写过滤器，只需按不同语种任意组合即可。
    # In addition to that, it supports abbreviation filters, allowing for any combination of different languages.
    # 示例：您可以任意指定多种组合，进行过滤
    # Example: You can specify any combination to filter

    # Langfilters = ["zh"]              # 按中文识别
    # Langfilters = ["en"]              # 按英文识别
    # Langfilters = ["ja"]              # 按日文识别
    # Langfilters = ["ko"]              # 按韩文识别
    # Langfilters = ["zh_ja"]           # 中日混合识别
    # Langfilters = ["zh_en"]           # 中英混合识别
    # Langfilters = ["ja_en"]           # 日英混合识别
    # Langfilters = ["zh_ko"]           # 中韩混合识别
    # Langfilters = ["ja_ko"]           # 日韩混合识别
    # Langfilters = ["en_ko"]           # 英韩混合识别
    # Langfilters = ["zh_ja_en"]        # 中日英混合识别
    # Langfilters = ["zh_ja_en_ko"]     # 中日英韩混合识别

    # 更多过滤组合，请您随意。。。For more filter combinations, please feel free to......


    # DEFINITION
    PARSE_TAG = re.compile(r'(⑥\$\d+[\d]{6,}⑥)')

    @staticmethod
    def _clears():
        LangSegment._text_cache = None
        LangSegment._text_lasts = None
        LangSegment._text_langs = None
        LangSegment._text_waits = None
        LangSegment._lang_count = None
        LangSegment._lang_eos   = None
        pass

    @staticmethod
    def _is_english_word(word):
        return bool(re.match(r'^[a-zA-Z]+$', word))

    @staticmethod
    def _is_chinese(word):
        for char in word:
            if '\u4e00' <= char <= '\u9fff':
                return True
        return False

    @staticmethod
    def _is_japanese_kana(word):
        pattern = re.compile(r'[\u3040-\u309F\u30A0-\u30FF]+')
        matches = pattern.findall(word)
        return len(matches) > 0

    @staticmethod
    def _insert_english_uppercase(word):
        modified_text = re.sub(r'(?<!\b)([A-Z])', r' \1', word)
        modified_text = modified_text.strip('-')
        return modified_text + " "

    @staticmethod
    def _saveData(words,language:str,text:str):
        # Language word statistics
        lang_count = LangSegment._lang_count
        if lang_count is None:lang_count = defaultdict(int)
        if not "|" in language:lang_count[language] += int(len(text)//2) if language == "en" else len(text)
        LangSegment._lang_count = lang_count
        # Merge the same language and save the results
        preData = words[-1] if len(words) > 0 else None
        if preData and  (preData["lang"] == language):
            text = preData["text"] + text
            preData["text"] = text
            return preData
        data = {"lang":language,"text": text}
        filters = LangSegment.Langfilters
        if filters is None or len(filters) == 0 or "?" in language or   \
            language in filters or language in filters[0] or \
            filters[0] == "*" or filters[0] in "alls-mixs-autos":
            words.append(data)
        return data

    @staticmethod
    def _addwords(words,language,text):
        if text is None or len(text.strip()) == 0:return True
        if language is None:language = ""
        language = language.lower()
        if language == 'en':text = LangSegment._insert_english_uppercase(text)
        # text = re.sub(r'[(（）)]', ',' , text) # Keep it.
        text_waits = LangSegment._text_waits
        ispre_waits = len(text_waits)>0
        preResult = text_waits.pop() if ispre_waits else None
        if preResult is None:preResult = words[-1] if len(words) > 0 else None
        if preResult and ("|" in preResult["lang"]):
            pre_lang = preResult["lang"]
            if language in pre_lang:preResult["lang"] = language = language.split("|")[0]
            else:preResult["lang"]=pre_lang.split("|")[0]
            if ispre_waits:preResult = LangSegment._saveData(words,preResult["lang"],preResult["text"])
        pre_lang = preResult["lang"] if preResult else None
        if ("|" in language) and (pre_lang and not pre_lang in language and not "…" in language):language = language.split("|")[0]
        filters = LangSegment.Langfilters
        if "|" in language:LangSegment._text_waits.append({"lang":language,"text": text})
        else:LangSegment._saveData(words,language,text)
        return False

    @staticmethod
    def _get_prev_data(words):
        data = words[-1] if words and len(words) > 0 else None
        if data:return (data["lang"] , data["text"])
        return (None,"")

    @staticmethod
    def _match_ending(input , index):
        if input is None or len(input) == 0:return False,None
        input = re.sub(r'\s+', '', input)
        if len(input) == 0 or abs(index) > len(input):return False,None
        ending_pattern = re.compile(r'([「」“”‘’"\':：。.！!?．？])')
        return ending_pattern.match(input[index]),input[index]

    @staticmethod
    def _cleans_text(cleans_text):
        cleans_text = re.sub(r'([^\w]+)', '', cleans_text)
        return cleans_text

    @staticmethod
    def _lang_classify(cleans_text):
        language, *_ = langid.classify(cleans_text)
        return language

    @staticmethod
    def _parse_language(words , segment):
        LANG_JA = "ja"
        LANG_ZH = "zh"
        language = LANG_ZH
        regex_pattern = re.compile(r'([^\w\s]+)')
        lines = regex_pattern.split(segment)
        lines_max = len(lines)
        LANG_EOS =LangSegment._lang_eos
        for index, text in enumerate(lines):
            if len(text) == 0:continue
            EOS = index >= (lines_max - 1)
            nextId = index + 1
            nextText = lines[nextId] if not EOS else ""
            nextPunc = len(re.sub(regex_pattern,'',re.sub(r'\n+','',nextText)).strip()) == 0
            textPunc = len(re.sub(regex_pattern,'',re.sub(r'\n+','',text)).strip()) == 0
            if not EOS and (textPunc == True or ( len(nextText.strip()) >= 0 and nextPunc == True)):
                lines[nextId] = f'{text}{nextText}'
                continue
            number_tags = re.compile(r'(⑥\d{6,}⑥)')
            cleans_text = re.sub(number_tags, '' ,text)
            cleans_text = LangSegment._cleans_text(cleans_text)
            language = LangSegment._lang_classify(cleans_text)
            prev_language , prev_text = LangSegment._get_prev_data(words)
            if len(cleans_text) <= 3 and LangSegment._is_chinese(cleans_text):
                if EOS and LANG_EOS: language = LANG_ZH if len(cleans_text) <= 1 else language
                elif LangSegment._is_japanese_kana(cleans_text):language = LANG_JA
                else:
                    LANG_UNKNOWN = f'{LANG_ZH}|{LANG_JA}'
                    match_end,match_char = LangSegment._match_ending(text, -1)
                    referen = prev_language in LANG_UNKNOWN or LANG_UNKNOWN in prev_language if prev_language else False
                    if match_char in "。.": language = prev_language if referen and len(words) > 0 else language
                    else:language = f"{LANG_UNKNOWN}|…"
            text,*_ = re.subn(number_tags , LangSegment._restore_number , text )
            LangSegment._addwords(words,language,text)
            pass
        pass

    @staticmethod
    def _restore_number(matche):
        value = matche.group(0)
        text_cache = LangSegment._text_cache
        if value in text_cache:
            process , data = text_cache[value]
            tag , match = data
            value = match
        return value

    @staticmethod
    def _pattern_symbols(item , text):
        if text is None:return text
        tag , pattern , process = item
        matches = pattern.findall(text)
        if len(matches) == 1 and "".join(matches[0]) == text:
            return text
        for i , match in enumerate(matches):
            key = f"⑥{tag}{i:06d}⑥"
            text = re.sub(pattern , key , text , count=1)
            LangSegment._text_cache[key] = (process , (tag , match))
        return text

    @staticmethod
    def _process_symbol(words,data):
        tag , match = data
        language = match[1]
        text = match[2]
        LangSegment._addwords(words,language,text)
        pass

    @staticmethod
    def _process_english(words,data):
        tag , match = data
        text = match[0]
        language = "en"
        LangSegment._addwords(words,language,text)
        pass

    @staticmethod
    def _process_korean(words,data):
        tag , match = data
        text = match[0]
        language = "ko"
        LangSegment._addwords(words,language,text)
        pass

    @staticmethod
    def _process_quotes(words,data):
        tag , match = data
        text = "".join(match)
        childs = LangSegment.PARSE_TAG.findall(text)
        if len(childs) > 0:
            LangSegment._process_tags(words , text , False)
        else:
            cleans_text = LangSegment._cleans_text(match[1])
            if len(cleans_text) <= 3:
                LangSegment._parse_language(words,text)
            else:
                language = LangSegment._lang_classify(cleans_text)
                LangSegment._addwords(words,language,text)
        pass

    @staticmethod
    def _process_number(words,data): # "$0" process only
        """
        Numbers alone cannot accurately identify language.
        Because numbers are universal in all languages.
        So it won't be executed here, just for testing.
        """
        tag , match = data
        language = words[0]["lang"] if len(words) > 0 else "zh"
        text = match
        LangSegment._addwords(words,language,text)
        pass

    @staticmethod
    def _process_tags(words , text , root_tag):
        text_cache = LangSegment._text_cache
        segments = re.split(LangSegment.PARSE_TAG, text)
        segments_len = len(segments) - 1
        for index , text in enumerate(segments):
            if root_tag:LangSegment._lang_eos = index >= segments_len
            if LangSegment.PARSE_TAG.match(text):
                process , data = text_cache[text]
                if process:process(words , data)
            else:
                LangSegment._parse_language(words , text)
            pass
        return words

    @staticmethod
    def _parse_symbols(text):
        TAG_NUM = "00" # "00" => default channels , "$0" => testing channel
        TAG_S1,TAG_P1,TAG_P2,TAG_EN,TAG_KO = "$1" ,"$2" ,"$3" ,"$4" ,"$5"
        process_list = [
            (  TAG_S1  , re.compile(LangSegment.SYMBOLS_PATTERN) , LangSegment._process_symbol  ),      # Symbol Tag
            (  TAG_KO  , re.compile('(([【《（(“‘"\']*(\d+\W*\s*)*[\uac00-\ud7a3]+[\W\s]*)+)')  , LangSegment._process_korean  ),      # Korean words
            (  TAG_NUM , re.compile(r'(\W*\d+\W+\d*\W*\d*)')        , LangSegment._process_number  ),      # Number words, Universal in all languages, Ignore it.
            (  TAG_EN  , re.compile(r'(([【《（(“‘"\']*[a-zA-Z]+[\W\s]*)+)')    , LangSegment._process_english ),                      # English words
            (  TAG_P1  , re.compile(r'(["\'])(.*?)(\1)')         , LangSegment._process_quotes  ),      # Regular quotes
            (  TAG_P2  , re.compile(r'([\n]*[【《（(“‘])([^【《（(“‘’”)）》】]{3,})([’”)）》】][\W\s]*[\n]{,1})')   , LangSegment._process_quotes  ),  # Special quotes, There are left and right.
        ]
        LangSegment._lang_eos = False
        text_cache = LangSegment._text_cache = {}
        for item in process_list:
            text = LangSegment._pattern_symbols(item , text)
        words = LangSegment._process_tags([] , text , True)
        lang_count = LangSegment._lang_count
        if lang_count and len(lang_count) > 0:
            lang_count = dict(sorted(lang_count.items(), key=lambda x: x[1], reverse=True))
            lang_count = list(lang_count.items())
            LangSegment._lang_count = lang_count
        return words

    @staticmethod
    def setfilters(filters):
        # 当过滤器更改时，清除缓存
        # When the filter changes, clear the cache
        if LangSegment.Langfilters != filters:
            LangSegment._clears()
            LangSegment.Langfilters = filters
        pass

    @staticmethod
    def getfilters():
        return LangSegment.Langfilters


    @staticmethod
    def getCounts():
        lang_count = LangSegment._lang_count
        if lang_count is not None:return lang_count
        text_langs = LangSegment._text_langs
        if text_langs is None or len(text_langs) == 0:return [("zh",0)]
        lang_counts = defaultdict(int)
        for d in text_langs:lang_counts[d['lang']] += int(len(d['text'])//2) if d['lang'] == "en" else len(d['text'])
        lang_counts = dict(sorted(lang_counts.items(), key=lambda x: x[1], reverse=True))
        lang_counts = list(lang_counts.items())
        LangSegment._lang_count = lang_counts
        return lang_counts

    @staticmethod
    def getTexts(text:str):
        if text is None or len(text.strip()) == 0:
            LangSegment._clears()
            return []
        # lasts
        text_langs = LangSegment._text_langs
        if LangSegment._text_lasts == text and text_langs is not None:return text_langs
        # parse
        LangSegment._text_waits = []
        LangSegment._lang_count = None
        LangSegment._text_lasts = text
        text = LangSegment._parse_symbols(text)
        LangSegment._text_langs = text
        return text

    @staticmethod
    def classify(text:str):
        return LangSegment.getTexts(text)

def setfilters(filters):
    """
    功能：语言过滤组功能, 可以指定保留语言。不在过滤组中的语言将被清除。您可随心搭配TTS语音合成所支持的语言。
    Function: Language filter group function, you can specify reserved languages. \n
    Languages not in the filter group will be cleared. You can match the languages supported by TTS Text To Speech as you like.\n
    Args:
        filters (list): ["zh", "en", "ja", "ko"]
    """
    LangSegment.setfilters(filters)
    pass

def getfilters():
    """
    功能：语言过滤组功能, 可以指定保留语言。不在过滤组中的语言将被清除。您可随心搭配TTS语音合成所支持的语言。
    Function: Language filter group function, you can specify reserved languages. \n
    Languages not in the filter group will be cleared. You can match the languages supported by TTS Text To Speech as you like.\n
    Args:
        filters (list): ["zh", "en", "ja", "ko"]
    """
    return LangSegment.getfilters()

# @Deprecated：Use shorter setfilters
def setLangfilters(filters):
    """
    >0.1.9废除：使用更简短的setfilters
    """
    setfilters(filters)
# @Deprecated：Use shorter getfilters
def getLangfilters():
    """
    >0.1.9废除：使用更简短的getfilters
    """
    return getfilters()

def getTexts(text:str):
    """
    功能：对输入的文本进行多语种分词\n
    Feature: Tokenizing multilingual text input.\n
    参数-Args:
        text (str): Text content,文本内容\n
    返回-Returns:
        list: 示例结果：[{'lang':'zh','text':'?'},...]\n
        lang=语种 , text=内容\n
    """
    return LangSegment.getTexts(text)

def getCounts():
    """
    功能：分词结果统计，按语种字数降序，用于确定其主要语言\n
    Function: Tokenizing multilingual text input.\n
    返回-Returns:
        list: 示例结果：[('zh', 5), ('ja', 2), ('en', 1)] = [(语种,字数含标点)]\n
    """
    return LangSegment.getCounts()

def classify(text:str):
    """
    功能：兼容接口实现
    Function: Compatible interface implementation
    """
    return LangSegment.classify(text)

def printList(langlist):
    """
    功能：打印数组结果
    Function: Print array results
    """
    print("\n\n===================【打印结果】===================")
    if langlist is None or len(langlist) == 0:
        print("无内容结果,No content result")
        return
    for line in langlist:
        print(line)
    pass



if __name__ == "__main__":

    # -----------------------------------
    # 更新日志：新版本分词更加精准。
    # Changelog: The new version of the word segmentation is more accurate.
    # -----------------------------------

    # 输入示例1：（包含日文，中文）
    # text = "“昨日は雨が降った，音楽、映画。。。”你今天学习日语了吗？春は桜の季節です。语种分词是语音合成必不可少的环节。言語分詞は音声合成に欠かせない環節である！"

    # 输入示例2：（包含日文，中文）
    # text = "欢迎来玩。東京，は日本の首都です。欢迎来玩.  太好了!"

    # 输入示例3：（包含日文，中文）
    # text = "明日、私たちは海辺にバカンスに行きます。你会说日语吗：“中国語、話せますか” 你的日语真好啊！"

    # 输入示例4：（包含日文，中文，韩语，英文）
    text = "你的名字叫<ja>佐々木？<ja>吗？韩语中的안녕 오빠读什么呢？あなたの体育の先生は誰ですか? 此次发布会带来了四款iPhone 15系列机型和三款Apple Watch等一系列新品，这次的iPad Air采用了LCD屏幕"

    # 进行分词：（接入TTS项目仅需一行代码调用）
    langlist = LangSegment.getTexts(text)
    printList(langlist)


    # 语种统计:
    print("\n===================【语种统计】===================")
    # 获取所有语种数组结果，根据内容字数降序排列
    langCounts = LangSegment.getCounts()
    print(langCounts , "\n")

    # 根据结果获取内容的主要语种 (语言，字数含标点)
    lang , count = langCounts[0]
    print(f"输入内容的主要语言为 = {lang} ，字数 = {count}")
    print("==================================================\n")


    # 分词输出：lang=语言，text=内容
    # ===================【打印结果】===================
    # {'lang': 'zh', 'text': '你的名字叫'}
    # {'lang': 'ja', 'text': '佐々木？'}
    # {'lang': 'zh', 'text': '吗？韩语中的'}
    # {'lang': 'ko', 'text': '안녕 오빠'}
    # {'lang': 'zh', 'text': '读什么呢？'}
    # {'lang': 'ja', 'text': 'あなたの体育の先生は誰ですか?'}
    # {'lang': 'zh', 'text': ' 此次发布会带来了四款'}
    # {'lang': 'en', 'text': 'i Phone  '}
    # {'lang': 'zh', 'text': '15系列机型和三款'}
    # {'lang': 'en', 'text': 'Apple Watch '}
    # {'lang': 'zh', 'text': '等一系列新品，这次的'}
    # {'lang': 'en', 'text': 'i Pad Air '}
    # {'lang': 'zh', 'text': '采用了'}
    # {'lang': 'en', 'text': 'L C D '}
    # {'lang': 'zh', 'text': '屏幕'}
    # ===================【语种统计】===================

    # ===================【语种统计】===================
    # [('zh', 51), ('ja', 19), ('en', 18), ('ko', 5)]

    # 输入内容的主要语言为 = zh ，字数 = 51
    # ==================================================



