//
//  campus_pilot
//  dio_response_interceptor.dart
//
//  Created by Ngonidzashe Mangudya on 11/19/24.
//  Copyright (c) 2024 Codecraft Solutions. All rights reserved.
//

import 'dart:async';

import 'package:dio/dio.dart';

class DioResponseInterceptor extends Interceptor {
  @override
  Future<void> onResponse(
    Response<dynamic> response,
    ResponseInterceptorHandler handler,
  ) async {
    return handler.next(response);
  }
}
