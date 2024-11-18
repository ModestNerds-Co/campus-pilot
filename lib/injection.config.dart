// GENERATED CODE - DO NOT MODIFY BY HAND

// **************************************************************************
// InjectableConfigGenerator
// **************************************************************************

// ignore_for_file: type=lint
// coverage:ignore-file

// ignore_for_file: no_leading_underscores_for_library_prefixes
import 'package:campus_pilot/core/services/network.dart' as _i333;
import 'package:campus_pilot/core/services/package.dart' as _i603;
import 'package:campus_pilot/core/services/scheduler.dart' as _i833;
import 'package:campus_pilot/core/services/secure_storage.dart' as _i1062;
import 'package:campus_pilot/core/services/storage.dart' as _i368;
import 'package:campus_pilot/core/state/connectivity_status/connectivity_status_bloc.dart'
    as _i940;
import 'package:campus_pilot/core/state/locale/locale_bloc.dart' as _i1073;
import 'package:get_it/get_it.dart' as _i174;
import 'package:injectable/injectable.dart' as _i526;

extension GetItInjectableX on _i174.GetIt {
// initializes the registration of main-scope dependencies inside of GetIt
  _i174.GetIt init({
    String? environment,
    _i526.EnvironmentFilter? environmentFilter,
  }) {
    final gh = _i526.GetItHelper(
      this,
      environment,
      environmentFilter,
    );
    gh.factory<_i1073.LocaleBloc>(() => _i1073.LocaleBloc());
    gh.factory<_i940.ConnectivityStatusBloc>(
        () => _i940.ConnectivityStatusBloc());
    gh.singletonAsync<_i368.StorageService>(
        () => _i368.StorageService.getInstance());
    gh.singletonAsync<_i1062.SecureStorageService>(
        () => _i1062.SecureStorageService.getInstance());
    gh.singletonAsync<_i603.PackageService>(
        () => _i603.PackageService.getInstance());
    gh.singletonAsync<_i333.NetworkService>(
        () => _i333.NetworkService.getInstance());
    gh.singletonAsync<_i833.SchedulingService>(
        () => _i833.SchedulingService.getInstance());
    return this;
  }
}
