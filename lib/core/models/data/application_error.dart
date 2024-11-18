//
//  campus_pilot
//  application_error.dart
//
//  Created by Ngonidzashe Mangudya on 11/19/24.
//  Copyright (c) 2024 Codecraft Solutions. All rights reserved.
//

class ApplicationError implements Exception {
  ApplicationError(
    this.message, {
    this.title = "Hey, it's not your fault",
  });

  factory ApplicationError.unknownError() => ApplicationError(
        'An unknown error occurred',
      );

  final String message;
  final String title;
}
