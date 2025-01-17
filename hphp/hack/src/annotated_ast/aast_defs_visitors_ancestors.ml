(*
 * Copyright (c) 2017, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

module SM = Ast_defs.ShapeMap
module LM = Local_id.Map

class virtual ['self] iter =
  object (self : 'self)
    inherit [_] Ast_defs.iter

    method private on_shape_map
        : 'a. ('env -> 'a -> unit) -> 'env -> 'a SM.t -> unit =
      (fun f env x -> SM.iter (self#on_shape_map_entry f env) x)

    method private on_shape_map_entry
        : 'a. ('env -> 'a -> unit) -> 'env -> SM.key -> 'a -> unit =
      fun f env key data ->
        self#on_shape_field_name env key;
        f env data

    method private on_local_id_map
        : 'a. ('env -> 'a -> unit) -> 'env -> 'a LM.t -> unit =
      (fun f env -> LM.iter (fun _ -> f env))

    method on_'ex _ _ = ()

    method on_'en _ _ = ()
  end

class virtual ['self] reduce =
  object (self : 'self)
    inherit [_] Ast_defs.reduce

    method private on_shape_map
        : 'a. ('env -> 'a -> 'acc) -> 'env -> 'a SM.t -> 'acc =
      fun f env x ->
        SM.fold
          (fun k d acc -> self#plus acc (self#on_shape_map_entry f env k d))
          x
          self#zero

    method private on_shape_map_entry
        : 'a. ('env -> 'a -> 'acc) -> 'env -> SM.key -> 'a -> 'acc =
      fun f env key data ->
        self#plus (self#on_shape_field_name env key) (f env data)

    method private on_local_id_map
        : 'a. ('env -> 'a -> 'acc) -> 'env -> 'a LM.t -> 'acc =
      fun f env x ->
        LM.fold (fun _ d acc -> self#plus acc (f env d)) x self#zero

    method on_'ex _env _ = self#zero

    method on_'en _env _ = self#zero
  end

class virtual ['self] map =
  object (self : 'self)
    inherit [_] Ast_defs.map

    method private on_shape_map
        : 'a 'b. ('env -> 'a -> 'b) -> 'env -> 'a SM.t -> 'b SM.t =
      fun f env x ->
        let map_entry key data acc =
          let key = self#on_shape_field_name env key in
          let data = f env data in
          SM.add key data acc
        in
        SM.fold map_entry x SM.empty

    method private on_local_id_map
        : 'a 'b. ('env -> 'a -> 'b) -> 'env -> 'a LM.t -> 'b LM.t =
      (fun f env -> LM.map (f env))
  end

class virtual ['self] endo =
  object (self : 'self)
    inherit [_] Ast_defs.endo

    method private on_shape_map
        : 'a 'b. ('env -> 'a -> 'b) -> 'env -> 'a SM.t -> 'b SM.t =
      fun f env x ->
        (* FIXME: Should be possible to write a true (more efficient) endo
           implementation rather than copying map *)
        let map_entry key data acc =
          let key = self#on_shape_field_name env key in
          let data = f env data in
          SM.add key data acc
        in
        SM.fold map_entry x SM.empty

    method private on_local_id_map
        : 'a 'b. ('env -> 'a -> 'b) -> 'env -> 'a LM.t -> 'b LM.t =
      (fun f env -> LM.map (f env))
  end
