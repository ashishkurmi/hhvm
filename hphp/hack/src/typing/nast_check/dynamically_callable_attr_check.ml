(*
 * Copyright (c) 2019, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

[@@@warning "-33"]

open Hh_prelude

[@@@warning "+33"]

open Hh_prelude
open Aast
module SN = Naming_special_names

let has_reified_generics tparaml =
  List.exists
    ~f:(fun tparam ->
      match tparam.tp_reified with
      | Erased -> false
      | SoftReified
      | Reified ->
        true)
    tparaml

let handler =
  object
    inherit Nast_visitor.handler_base

    method! at_method_ _ m =
      let (pos, _) = m.m_name in
      let vis = m.m_visibility in
      let attr = m.m_user_attributes in
      match
        Naming_attributes.mem_pos SN.UserAttributes.uaDynamicallyCallable attr
      with
      | Some p ->
        (if not (Aast.equal_visibility vis Public) then
          let vis =
            match vis with
            | Public -> `public
            | Private -> `private_
            | Protected -> `protected
            | Internal -> `internal
          in
          Errors.add_naming_error
          @@ Naming_error.Illegal_use_of_dynamically_callable
               { attr_pos = p; meth_pos = pos; vis });
        if has_reified_generics m.m_tparams then
          Errors.add_nast_check_error
          @@ Nast_check_error.Dynamically_callable_reified p;
        ()
      | _ -> ()

    method! at_fun_ _ f =
      let attrs = f.f_user_attributes in
      match
        Naming_attributes.mem_pos SN.UserAttributes.uaDynamicallyCallable attrs
      with
      | Some p ->
        if has_reified_generics f.f_tparams then
          Errors.add_nast_check_error
          @@ Nast_check_error.Dynamically_callable_reified p;
        ()
      | _ -> ()
  end
