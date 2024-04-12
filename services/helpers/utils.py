#
#  utils.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

import re
from typing import Union


def clean_text_to_float(value) -> Union[None, float]:
    """
    Takes any value, converts it to a string, and extracts any sequence of digits possibly containing a decimal point.

    Args:
    value (Any): The input value to be formatted.

    Returns:
    Union[None, float]: The formatted value.
    """
    if value is not None:
        # This regular expression matches any sequence of digits possibly containing a decimal point
        match = re.search(r"\d+(\.\d+)?", str(value))
        if match:
            return float(match.group())
    return None


def format_text_to_displayable_text(text) -> str:
    """
    Takes any string, converts it to title case, and replaces all underscores with spaces.

    Args:
    text (str): The input string to be formatted.

    Returns:
    str: The formatted string.
    """
    text = re.sub(r"_([a-z]?)", lambda match: " " + match.group(1).upper(), text)
    return text.title()


def string_to_key_format(input_string: str) -> str:
    """
    Takes any string, converts it to lowercase, trims it, and replaces all spaces with underscores.

    Args:
    input_string (str): The input string to be formatted.

    Returns:
    str: The formatted string.
    """
    return input_string.lower().strip().replace(" ", "_")
