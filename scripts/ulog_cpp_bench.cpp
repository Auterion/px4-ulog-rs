// Benchmark harness for PX4/ulog_cpp. Reads a ULog file N times through the
// streaming reader and prints min/mean elapsed milliseconds plus throughput.
// Invoked by scripts/benchmark_compare.sh.
//
// SPDX-License-Identifier: BSD-3-Clause

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <memory>
#include <string>
#include <sys/stat.h>
#include <vector>

#include <ulog_cpp/data_container.hpp>
#include <ulog_cpp/reader.hpp>

namespace {

// Minimal DataContainer subclass that swallows parse errors silently so we
// measure parse speed on every fixture (including truncated_real.ulg) without
// being thrown off by stderr spam.
class SilentDataContainer : public ulog_cpp::DataContainer {
 public:
  SilentDataContainer()
      : ulog_cpp::DataContainer(ulog_cpp::DataContainer::StorageConfig::Header) {}
  void error(const std::string& /*msg*/, bool /*is_recoverable*/) override {}
};

long file_size(const std::string& path) {
  struct stat st;
  if (::stat(path.c_str(), &st) != 0) return -1;
  return static_cast<long>(st.st_size);
}

double parse_once(const std::string& path) {
  auto container = std::make_shared<SilentDataContainer>();
  ulog_cpp::Reader reader{container};

  std::ifstream in{path, std::ios::binary};
  if (!in) {
    std::fprintf(stderr, "cannot open %s\n", path.c_str());
    std::exit(1);
  }

  constexpr size_t BUF = 64 * 1024;
  std::vector<uint8_t> buf(BUF);

  const auto t0 = std::chrono::steady_clock::now();
  while (in) {
    in.read(reinterpret_cast<char*>(buf.data()), buf.size());
    const auto n = in.gcount();
    if (n > 0) reader.readChunk(buf.data(), static_cast<int>(n));
  }
  const auto t1 = std::chrono::steady_clock::now();
  return std::chrono::duration<double, std::milli>(t1 - t0).count();
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 2) {
    std::fprintf(stderr, "usage: %s <file.ulg> [file.ulg ...]\n", argv[0]);
    return 2;
  }

  std::printf("%-45s %8s %10s %12s\n", "File", "Size", "Time(ms)", "MB/s");
  std::puts("--------------------------------------------------------------------------------");

  double total_ms = 0.0;
  long total_bytes = 0;
  const int iters = 10;

  for (int i = 1; i < argc; ++i) {
    const std::string path = argv[i];
    const long size = file_size(path);
    if (size < 0) {
      std::fprintf(stderr, "skip: %s\n", path.c_str());
      continue;
    }
    // Warmup.
    parse_once(path);
    parse_once(path);

    double sum = 0.0;
    for (int r = 0; r < iters; ++r) sum += parse_once(path);
    const double mean_ms = sum / iters;
    const double size_mb = size / (1024.0 * 1024.0);
    const double mb_per_s = size_mb / (mean_ms / 1000.0);

    std::printf("%-45s %7.1fM %9.2fms %10.1f MB/s\n",
                path.c_str(), size_mb, mean_ms, mb_per_s);
    total_ms += mean_ms;
    total_bytes += size;
  }

  const double total_mb = total_bytes / (1024.0 * 1024.0);
  std::puts("--------------------------------------------------------------------------------");
  std::printf("%-45s %7.1fM %9.2fms %10.1f MB/s\n", "TOTAL", total_mb, total_ms,
              total_mb / (total_ms / 1000.0));
  return 0;
}
