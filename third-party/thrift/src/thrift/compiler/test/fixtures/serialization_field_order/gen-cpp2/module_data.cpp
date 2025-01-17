/**
 * Autogenerated by Thrift for src/module.thrift
 *
 * DO NOT EDIT UNLESS YOU ARE SURE THAT YOU KNOW WHAT YOU ARE DOING
 *  @generated @nocommit
 */

#include "thrift/compiler/test/fixtures/serialization_field_order/gen-cpp2/module_data.h"

#include <thrift/lib/cpp2/gen/module_data_cpp.h>

namespace apache {
namespace thrift {

const std::array<folly::StringPiece, 3> TStructDataStorage<::cpp2::Foo>::fields_names = {{
  "field1",
  "field2",
  "field3",
}};
const std::array<int16_t, 3> TStructDataStorage<::cpp2::Foo>::fields_ids = {{
  3,
  1,
  2,
}};
const std::array<protocol::TType, 3> TStructDataStorage<::cpp2::Foo>::fields_types = {{
  TType::T_I32,
  TType::T_I32,
  TType::T_I32,
}};

const std::array<folly::StringPiece, 3> TStructDataStorage<::cpp2::Foo2>::fields_names = {{
  "field1",
  "field2",
  "field3",
}};
const std::array<int16_t, 3> TStructDataStorage<::cpp2::Foo2>::fields_ids = {{
  3,
  1,
  2,
}};
const std::array<protocol::TType, 3> TStructDataStorage<::cpp2::Foo2>::fields_types = {{
  TType::T_I32,
  TType::T_I32,
  TType::T_I32,
}};

} // namespace thrift
} // namespace apache
