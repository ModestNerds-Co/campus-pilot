#
#  generate_random_numbers.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import random
import string


def generate_random_numbers(n: int = 15) -> str:
    characters = string.digits
    random_numbers = "".join(random.choice(characters) for _ in range(n))
    return random_numbers
