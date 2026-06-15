// Read ULog files with PX4/ulog_cpp and report any parse errors. Used to
// cross-check what ulog_cpp does on inputs where px4-ulog-rs returned an
// error. Input is the same list-of-paths format as ulog_cpp_bench but the
// output is one line per file:
//
//   OK  <path>
//   ERR <path>  <error message>   [one line per error]
//
// SPDX-License-Identifier: BSD-3-Clause

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <memory>
#include <string>
#include <vector>

#include <ulog_cpp/data_container.hpp>
#include <ulog_cpp/reader.hpp>

namespace {

class ReportingDataContainer : public ulog_cpp::DataContainer {
 public:
  ReportingDataContainer()
      : ulog_cpp::DataContainer(ulog_cpp::DataContainer::StorageConfig::Header) {}
  void error(const std::string& msg, bool /*is_recoverable*/) override {
    errors.push_back(msg);
  }
  std::vector<std::string> errors;
};

bool parse_one(const std::string& path, std::vector<std::string>& errors) {
  auto container = std::make_shared<ReportingDataContainer>();
  ulog_cpp::Reader reader{container};

  std::ifstream in{path, std::ios::binary};
  if (!in) {
    errors.push_back("cannot open file");
    return false;
  }

  constexpr size_t BUF = 64 * 1024;
  std::vector<uint8_t> buf(BUF);
  bool threw = false;
  try {
    while (in) {
      in.read(reinterpret_cast<char*>(buf.data()), buf.size());
      const auto n = in.gcount();
      if (n > 0) reader.readChunk(buf.data(), static_cast<int>(n));
    }
  } catch (const std::exception& e) {
    errors.push_back(std::string("exception: ") + e.what());
    threw = true;
  } catch (...) {
    errors.push_back("exception: <unknown>");
    threw = true;
  }

  errors.insert(errors.end(), container->errors.begin(), container->errors.end());
  return !threw && container->errors.empty();
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::fprintf(stderr, "usage: %s <file.ulg> [file.ulg ...]\n", argv[0]);
    return 2;
  }
  int n_ok = 0, n_err = 0;
  for (int i = 1; i < argc; ++i) {
    std::vector<std::string> errors;
    const bool ok = parse_one(argv[i], errors);
    if (ok) {
      std::printf("OK\t%s\n", argv[i]);
      ++n_ok;
    } else {
      ++n_err;
      if (errors.empty()) {
        std::printf("ERR\t%s\t<no message>\n", argv[i]);
      } else {
        for (const auto& e : errors) {
          std::string one_line;
          one_line.reserve(e.size());
          for (char c : e) one_line.push_back(c == '\n' ? ' ' : c);
          std::printf("ERR\t%s\t%s\n", argv[i], one_line.c_str());
        }
      }
    }
  }
  std::fprintf(stderr, "\n%d ok, %d err\n", n_ok, n_err);
  return 0;
}
