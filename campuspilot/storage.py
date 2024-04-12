#
#  storage.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


from storages.backends.s3boto3 import S3Boto3Storage


class StaticStorage(S3Boto3Storage):
    location = "static"
    default_acl = "public-read"
    querystring_auth = False


class PublicMediaStorage(S3Boto3Storage):
    location = "media"
    default_acl = "public-read"
    file_overwrite = False
    querystring_auth = False
