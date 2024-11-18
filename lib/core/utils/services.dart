//
//  campus_pilot
//  services.dart
//
//  Created by Ngonidzashe Mangudya on 11/19/24.
//  Copyright (c) 2024 Codecraft Solutions. All rights reserved.
//

import 'package:dio/dio.dart';

import '../../injection.dart';
import '../services/network.dart';
import '../services/scheduler.dart';
import '../services/secure_storage.dart';
import '../services/storage.dart';

SecureStorageService get $secureStorage => getIt<SecureStorageService>();

StorageService get $storage => getIt<StorageService>();

Dio get $dio => getIt<Dio>();

NetworkService get $network => getIt<NetworkService>();

SchedulingService get $scheduler => getIt<SchedulingService>();
