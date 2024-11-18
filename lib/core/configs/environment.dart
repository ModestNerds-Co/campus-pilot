//
//  campus_pilot
//  environment.dart
//
//  Created by Ngonidzashe Mangudya on 11/19/24.
//  Copyright (c) 2024 Codecraft Solutions. All rights reserved.
//

enum Environment {
  development(
    name: 'Development',
    shortName: 'dev',
    baseUrl: '',
  ),
  production(
    name: 'Production',
    shortName: 'prod',
    baseUrl: '',
  );

  const Environment({
    required this.name,
    required this.shortName,
    required this.baseUrl,
  });

  final String name;
  final String shortName;
  final String baseUrl;

  @override
  String toString() => name;
}
