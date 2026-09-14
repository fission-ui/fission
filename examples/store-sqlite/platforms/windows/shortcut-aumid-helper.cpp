#include <windows.h>

#include <cwchar>
#include <cwctype>
#include <cstdio>

#include <propkey.h>
#include <propvarutil.h>
#include <shobjidl.h>
#include <wrl/client.h>

namespace {

using Microsoft::WRL::ComPtr;

bool IsValidAppUserModelId(const wchar_t* app_user_model_id) {
  if (app_user_model_id == nullptr) {
    return false;
  }

  const size_t length = std::wcslen(app_user_model_id);
  if (length == 0 || length > 128) {
    return false;
  }

  for (size_t index = 0; index < length; ++index) {
    if (std::iswspace(app_user_model_id[index]) != 0) {
      return false;
    }
  }

  return true;
}

int ReportFailure(const wchar_t* operation, HRESULT result) {
  std::fwprintf(
      stderr,
      L"%ls failed (HRESULT 0x%08lX).\n",
      operation,
      static_cast<unsigned long>(result));
  return 1;
}

}  // namespace

int wmain(int argc, wchar_t** argv) {
  if (argc != 3) {
    std::fwprintf(
        stderr,
        L"Usage: fission-shortcut-aumid.exe <shortcut.lnk> <app-user-model-id>\n");
    return 2;
  }

  const wchar_t* shortcut_path = argv[1];
  const wchar_t* app_user_model_id = argv[2];
  if (!IsValidAppUserModelId(app_user_model_id)) {
    std::fwprintf(
        stderr,
        L"The AppUserModelID must contain 1-128 UTF-16 code units and no whitespace.\n");
    return 3;
  }

  const HRESULT initialize_result =
      CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  const bool should_uninitialize = SUCCEEDED(initialize_result);
  if (FAILED(initialize_result) && initialize_result != RPC_E_CHANGED_MODE) {
    return ReportFailure(L"CoInitializeEx", initialize_result);
  }

  int exit_code = 0;
  ComPtr<IShellLinkW> shell_link;
  HRESULT result = CoCreateInstance(
      CLSID_ShellLink,
      nullptr,
      CLSCTX_INPROC_SERVER,
      IID_PPV_ARGS(&shell_link));
  if (FAILED(result)) {
    exit_code = ReportFailure(L"CoCreateInstance(CLSID_ShellLink)", result);
    goto finish;
  }

  {
    ComPtr<IPersistFile> persist_file;
    result = shell_link.As(&persist_file);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"QueryInterface(IPersistFile)", result);
      goto finish;
    }

    result = persist_file->Load(shortcut_path, STGM_READWRITE);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPersistFile::Load", result);
      goto finish;
    }

    ComPtr<IPropertyStore> property_store;
    result = shell_link.As(&property_store);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"QueryInterface(IPropertyStore)", result);
      goto finish;
    }

    PROPVARIANT app_id_value;
    PropVariantInit(&app_id_value);
    result = InitPropVariantFromString(app_user_model_id, &app_id_value);
    if (SUCCEEDED(result)) {
      result = property_store->SetValue(PKEY_AppUserModel_ID, app_id_value);
    }
    if (SUCCEEDED(result)) {
      result = property_store->Commit();
    }
    PropVariantClear(&app_id_value);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPropertyStore::SetValue/Commit", result);
      goto finish;
    }

    result = persist_file->Save(shortcut_path, TRUE);
    if (FAILED(result)) {
      exit_code = ReportFailure(L"IPersistFile::Save", result);
      goto finish;
    }
  }

finish:
  if (should_uninitialize) {
    CoUninitialize();
  }
  return exit_code;
}
